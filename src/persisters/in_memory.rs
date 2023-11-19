use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::definitions::saga::Saga;

use super::persister::{
    DefinitionPersister, InitialDataPersister, LockScope, LockType, PersistError,
};

#[derive(Debug, Clone)]
pub struct InMemoryPersister {
    sagas: Arc<RwLock<HashMap<Uuid, Saga>>>,
    locks: Arc<RwLock<HashMap<Uuid, ExecutingContext>>>,
    lock_timeout: Duration,
}

impl InMemoryPersister {
    pub fn new(lock_timeout: Duration) -> Self {
        Self {
            sagas: Arc::new(RwLock::new(Default::default())),
            locks: Arc::new(RwLock::new(Default::default())),
            lock_timeout,
        }
    }
}

impl DefinitionPersister for InMemoryPersister {
    fn retrieve(&self, id: Uuid) -> Result<Saga, PersistError> {
        self.sagas
            .read()
            .expect("sagas lock")
            .get(&id)
            .cloned()
            .ok_or(PersistError::NotFound)
    }

    fn store(&self, saga: Saga) -> Result<(), PersistError> {
        self.sagas
            .write()
            .expect("sagas lock")
            .insert(saga.id, saga);
        Ok(())
    }

    fn lock(&self, scope: LockScope, lock_type: LockType) -> Result<(), PersistError> {
        let insert = if let Some(context) = self
            .locks
            .read()
            .expect("persister locks lock")
            .get(&scope.id)
        {
            scope.executor_id == context.executor_id
                || matches!(context.lock_type, LockType::Failed)
                || context.instant_started.elapsed() > self.lock_timeout
        } else {
            true
        };

        if insert {
            self.locks.write().expect("persister locks lock").insert(
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
        &self,
        duration: Duration,
    ) -> Result<Option<(Uuid, String, Uuid)>, PersistError> {
        let new_executor = Uuid::new_v4();
        let scope_result = self
            .locks
            .read()
            .expect("persister locks lock")
            .iter()
            .find(|(_, context)| match context.lock_type {
                LockType::Failed => true,
                LockType::Finished => false,
                _ => context.instant_started.elapsed() > duration,
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
}

impl InitialDataPersister for InMemoryPersister {
    fn save_initial_state<S: serde::Serialize>(
        &self,
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

#[derive(Debug)]
struct ExecutingContext {
    executor_id: Uuid,
    lock_type: LockType,
    instant_started: Instant,
    name: String,
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    #[test]
    fn test_same_executor_can_always_lock() {
        let persister = InMemoryPersister::new(Duration::from_millis(10));
        let scope = LockScope {
            id: Uuid::new_v4(),
            executor_id: Uuid::new_v4(),
            name: "test1".to_string(),
        };
        persister.lock(scope.clone(), LockType::Initial).unwrap();
        persister.lock(scope.clone(), LockType::Failed).unwrap();
        persister.lock(scope.clone(), LockType::Retry).unwrap();
        persister.lock(scope.clone(), LockType::Executing).unwrap();
        persister.lock(scope, LockType::Finished).unwrap();
    }

    #[test]
    fn test_different_executor_can_lock_conditionally() {
        let persister = InMemoryPersister::new(Duration::from_millis(10));
        let scope1 = LockScope {
            id: Uuid::new_v4(),
            executor_id: Uuid::new_v4(),
            name: "test1".to_string(),
        };
        let scope2 = LockScope {
            id: scope1.id,
            executor_id: Uuid::new_v4(),
            name: "test1".to_string(),
        };
        persister.lock(scope1.clone(), LockType::Initial).unwrap();

        let result = persister.lock(scope2.clone(), LockType::Executing);
        assert!(matches!(result, Err(PersistError::Locked)));

        sleep(Duration::from_millis(13));

        let result = persister.lock(scope2.clone(), LockType::Failed);
        assert!(result.is_ok(), "{result:?}");

        let result = persister.lock(scope1.clone(), LockType::Executing);
        assert!(result.is_ok(), "{result:?}");
    }
}
