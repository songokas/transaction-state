use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::definitions::saga::Saga;

use super::persister::{DefinitionPersister, LockScope, LockType, PersistError};

#[derive(Debug, Default)]
pub struct InMemoryPersister {
    sagas: HashMap<Uuid, Saga>,
    locks: HashMap<Uuid, ExecutingContext>,
}

#[derive(Debug)]
struct ExecutingContext {
    executor_id: Uuid,
    lock_type: LockType,
    instant_started: Instant,
    name: String,
}

impl DefinitionPersister for InMemoryPersister {
    fn retrieve(&self, id: Uuid) -> Result<Saga, PersistError> {
        self.sagas.get(&id).cloned().ok_or(PersistError::NotFound)
    }

    fn store(&mut self, saga: Saga) -> Result<(), PersistError> {
        self.sagas.insert(saga.id, saga);
        Ok(())
    }

    fn lock(&mut self, scope: LockScope, lock_type: LockType) -> Result<(), PersistError> {
        let insert = if let Some(context) = self.locks.get(&scope.id) {
            scope.executor_id == context.executor_id
        } else {
            true
        };

        if insert {
            self.locks.insert(
                scope.id,
                ExecutingContext {
                    executor_id: scope.executor_id,
                    lock_type,
                    instant_started: Instant::now(),
                    name: scope.name,
                },
            );
            Ok(())
        } else {
            Err(PersistError::Locked)
        }
    }

    fn get_next_failed(
        &mut self,
        duration: Duration,
    ) -> Result<Option<(Uuid, String, Uuid)>, PersistError> {
        let new_executor = Uuid::new_v4();
        let scope_result = self
            .locks
            .iter()
            .find(|(_, context)| match context.lock_type {
                LockType::Failed => true,
                LockType::Initial if context.instant_started.elapsed() > duration => true,
                LockType::Retry if context.instant_started.elapsed() > duration => true,
                _ => false,
            })
            .map(|(key, context)| LockScope {
                id: *key,
                executor_id: new_executor,
                name: context.name.clone(),
            });
        if let Some(scope) = scope_result {
            self.lock(scope.clone(), LockType::Retry)?;
            Ok(Some((scope.id, scope.name, new_executor)))
        } else {
            Ok(None)
        }
    }

    fn save_initial_state<S: serde::Serialize>(
        &mut self,
        scope: LockScope,
        initial_state: &S,
    ) -> Result<(), PersistError> {
        let mut saga = Saga::new(scope.id);
        let state = serde_json::to_string(initial_state).expect("initial state serialize");
        saga.states.insert(0, state);
        self.store(saga)?;
        self.lock(scope, LockType::Initial)
    }
}
