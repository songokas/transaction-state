use std::{
    error::Error,
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use serde::{de::DeserializeOwned, Serialize};

use crate::persisters::persister::{DefinitionPersister, LockScope, LockType, PersistError};

use super::saga::Saga;

pub type OperationDefinition<State, In, OperationResult, E> = Box<
    dyn FnOnce(
            In,
        ) -> (
            State,
            Pin<Box<dyn Future<Output = Result<OperationResult, E>> + Send>>,
        ) + Send,
>;

pub struct SagaDefinition<State, In, OperationResult, E, Persister> {
    pub lock_scope: LockScope,
    step: u8,
    operation: OperationDefinition<Arc<State>, In, OperationResult, E>,
    persister: Persister,
    existing_saga: Arc<RwLock<Saga>>,
}

impl<
        State: 'static + Send + Sync,
        FactoryData: 'static + Send,
        OperationResult: 'static + Send,
        WrappingError: 'static + Send,
        Persister: 'static + Send,
    > SagaDefinition<State, FactoryData, OperationResult, WrappingError, Persister>
{
    pub fn new<StateCreator>(
        lock_scope: LockScope,
        state: StateCreator,
        initial_data: OperationResult,
        persister: Persister,
    ) -> Self
    where
        Persister: DefinitionPersister + Clone + Send,
        StateCreator: FnOnce(FactoryData) -> State + Send + 'static,
        FactoryData: Serialize,
        WrappingError: From<serde_json::Error> + From<PersistError>,
    {
        let existing_saga = Arc::new(RwLock::new(Saga::new(lock_scope.id)));
        let step = 0;
        let persist = persister.clone();
        Self {
            lock_scope,
            step,
            existing_saga: existing_saga.clone(),
            operation: Box::new(move |fd| {
                let persist_callback = |fd| {
                    let initial_state = serde_json::to_string(&fd).map_err(WrappingError::from)?;
                    let mut saga = existing_saga.write().expect("existing saga");
                    saga.states.insert(step, initial_state);
                    persist.store(saga.clone()).map_err(WrappingError::from)?;
                    Ok(())
                };

                let persit_result = persist_callback(&fd);
                let s = state(fd);
                let f = Box::pin(async move { persit_result.map(|_| initial_data) });
                (Arc::new(s), f)
            }),
            persister,
        }
    }

    pub fn step<
        NewError,
        F,
        FactoryResult: Send + 'static,
        Factory,
        Operation,
        NewFutureResult: 'static,
    >(
        self,
        operation: Operation,
        factory: Factory,
    ) -> SagaDefinition<State, FactoryData, NewFutureResult, WrappingError, Persister>
    where
        Operation: FnOnce(FactoryResult) -> F + Send + 'static,
        Factory: FnOnce(&State, OperationResult) -> FactoryResult + Send + 'static,
        F: Future<Output = Result<NewFutureResult, NewError>> + Send + 'static,
        WrappingError:
            Error + From<NewError> + From<serde_json::Error> + From<PersistError> + Send + 'static,
        NewFutureResult: Serialize + DeserializeOwned + Send,
        Persister: DefinitionPersister + Clone + Send + 'static,
    {
        let previous = self.operation;
        let persister = self.persister.clone();
        let definition_step = self.step + 1;
        let existing_saga = self.existing_saga.clone();
        SagaDefinition {
            lock_scope: self.lock_scope.clone(),
            step: definition_step,
            existing_saga: self.existing_saga.clone(),
            operation: Box::new(move |d| {
                let (current_state, previous_executing) = previous(d);

                let s = current_state.clone();
                let f = Box::pin(async move {
                    let operation_result = previous_executing.await?;
                    let factory_result = factory(&s, operation_result);
                    let existing_state = {
                        existing_saga
                            .read()
                            .expect("existing saga")
                            .states
                            .get(&definition_step)
                            .map(|s| serde_json::from_str(s))
                    };

                    if let Some(new_operation_result) = existing_state {
                        new_operation_result.map_err(WrappingError::from)
                    } else {
                        let new_operation_result =
                            operation(factory_result).await.map_err(WrappingError::from);

                        if let Ok(r) = &new_operation_result {
                            let state = serde_json::to_string(r).map_err(WrappingError::from)?;
                            let mut saga = existing_saga.write().expect("existing saga");
                            saga.states.insert(definition_step, state);
                            persister.store(saga.clone()).map_err(WrappingError::from)?;
                        }
                        new_operation_result
                    }
                });
                (current_state, f)
            }),
            persister: self.persister,
        }
    }

    pub async fn run(self, data: FactoryData) -> Result<OperationResult, WrappingError>
    where
        Persister: DefinitionPersister + Send,
        OperationResult: Send,
        WrappingError: From<PersistError> + Send,
        FactoryData: Send,
    {
        let lock_scope = self.lock_scope;
        self.persister
            .lock(lock_scope.clone(), LockType::Executing)
            .map_err(WrappingError::from)?;

        let (_state, f) = (self.operation)(data);
        let result = f.await;

        self.persister
            .lock(
                lock_scope,
                if result.is_ok() {
                    LockType::Finished
                } else {
                    LockType::Failed
                },
            )
            .map_err(WrappingError::from)?;
        result
    }

    pub async fn continue_from_last_step(self) -> Result<OperationResult, WrappingError>
    where
        Persister: DefinitionPersister + Send,
        OperationResult: Send,
        WrappingError: From<PersistError> + From<serde_json::Error> + Send,
        FactoryData: DeserializeOwned + Send,
    {
        let lock_scope = self.lock_scope;
        let saga = {
            self.persister
                .lock(lock_scope.clone(), LockType::Executing)
                .map_err(WrappingError::from)?;
            self.persister
                .retrieve(lock_scope.id)
                .map_err(WrappingError::from)?
        };

        let state = saga.states.get(&0).ok_or(PersistError::NotFound)?;
        let data = serde_json::from_str(state).map_err(WrappingError::from)?;
        *self.existing_saga.write().expect("saga lock") = saga;

        let (_state, f) = (self.operation)(data);
        let result = f.await;

        self.persister
            .lock(
                lock_scope,
                if result.is_ok() {
                    LockType::Finished
                } else {
                    LockType::Failed
                },
            )
            .map_err(WrappingError::from)?;
        result
    }
}
