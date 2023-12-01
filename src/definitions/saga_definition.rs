use std::{
    error::Error,
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use crate::persisters::persister::{LockScope, LockType, PersistError, StepPersister};

use super::saga_state::SagaState;

#[async_trait]
pub trait SagaRunner<In, Out, WrappingError> {
    async fn run(self, data: In) -> Result<Out, WrappingError>;
    async fn continue_from_last_step(self) -> Result<Out, WrappingError>
    where
        In: DeserializeOwned;
}

pub type OperationDefinition<State, In, Out, E> =
    Box<dyn FnOnce(In) -> (State, Pin<Box<dyn Future<Output = Result<Out, E>> + Send>>) + Send>;

pub struct SagaDefinition<State, In, Out, WrappingError, Persister> {
    lock_scope: LockScope,
    step: u8,
    operation: OperationDefinition<Arc<State>, In, Out, WrappingError>,
    persister: Persister,
    existing_saga: Arc<RwLock<SagaState>>,
}

impl<State, FactoryData, OperationResult, WrappingError, Persister>
    SagaDefinition<State, FactoryData, OperationResult, WrappingError, Persister>
where
    State: 'static + Send + Sync,
    FactoryData: Serialize + 'static + Send + Sync,
    OperationResult: 'static + Send,
    WrappingError: Error + From<PersistError> + 'static + Send + Sync,
    Persister: StepPersister + Clone + Send + Sync + 'static,
{
    pub fn new<StateCreator>(
        lock_scope: LockScope,
        state: StateCreator,
        initial_data: OperationResult,
        persister: Persister,
    ) -> Self
    where
        StateCreator: FnOnce(FactoryData, &OperationResult) -> State + Send + 'static,
    {
        let existing_saga = Arc::new(RwLock::new(SagaState::new(lock_scope.id)));
        let step = 0;
        let persist = persister.clone();
        let scope_id = lock_scope.id;
        Self {
            lock_scope,
            step,
            existing_saga: existing_saga.clone(),
            operation: Box::new(move |fd| {
                let initial_state = serde_json::to_string(&fd)
                    .map_err(PersistError::from)
                    .map_err(WrappingError::from);

                // TODO is there a need for initial_data
                let s = state(fd, &initial_data);
                let f = Box::pin(async move {
                    if !existing_saga
                        .read()
                        .expect("existing saga")
                        .states
                        .contains_key(&step)
                    {
                        persist
                            .store(scope_id, step, initial_state?)
                            .await
                            .map_err(WrappingError::from)?;
                    }

                    Ok(initial_data)
                });
                (Arc::new(s), f)
            }),
            persister,
        }
    }

    pub fn step<
        NewError,
        OperationFuture,
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
        Operation: FnOnce(FactoryResult) -> OperationFuture + Send + 'static,
        Factory: FnOnce(&State, OperationResult) -> FactoryResult + Send + 'static,
        OperationFuture: Future<Output = Result<NewFutureResult, NewError>> + Send + 'static,
        WrappingError: From<NewError> + From<PersistError>,
        NewFutureResult: Serialize + DeserializeOwned + Send + Sync,
    {
        let previous = self.operation;
        let persister = self.persister.clone();
        let definition_step = self.step + 1;
        let existing_saga = self.existing_saga.clone();
        let scope_id = self.lock_scope.id;
        SagaDefinition {
            lock_scope: self.lock_scope,
            step: definition_step,
            existing_saga: self.existing_saga.clone(),
            operation: Box::new(move |d| {
                let (current_state, previous_executing) = previous(d);

                let s = current_state.clone();
                let f = Box::pin(async move {
                    log::trace!("executing step {definition_step}");
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
                        new_operation_result
                            .map_err(PersistError::from)
                            .map_err(WrappingError::from)
                    } else {
                        let new_operation_result =
                            operation(factory_result).await.map_err(WrappingError::from);

                        if let Ok(r) = &new_operation_result {
                            let state = serde_json::to_string(r)
                                .map_err(PersistError::from)
                                .map_err(WrappingError::from)?;
                            persister
                                .store(scope_id, definition_step, state)
                                .await
                                .map_err(WrappingError::from)?;
                        }
                        new_operation_result
                    }
                });
                (current_state, f)
            }),
            persister: self.persister,
        }
    }

    pub fn on_error<
        NewError,
        OperationFuture,
        FactoryResult: Send + 'static,
        Factory,
        Operation,
        NewFutureResult,
    >(
        self,
        operation: Operation,
        factory: Factory,
    ) -> SagaDefinition<State, FactoryData, OperationResult, WrappingError, Persister>
    where
        Operation: FnOnce(FactoryResult) -> OperationFuture + Send + 'static,
        Factory: FnOnce(&State, &WrappingError) -> FactoryResult + Send + 'static,
        OperationFuture: Future<Output = Result<NewFutureResult, NewError>> + Send + 'static,
        WrappingError: From<NewError> + From<PersistError>,
        NewFutureResult: Serialize + DeserializeOwned + Send + Sync,
    {
        let previous = self.operation;
        let definition_step = self.step + 1;
        let existing_saga = self.existing_saga.clone();
        SagaDefinition {
            lock_scope: self.lock_scope,
            step: definition_step,
            existing_saga: self.existing_saga.clone(),
            operation: Box::new(move |d| {
                let (current_state, previous_executing) = previous(d);

                let s = current_state.clone();
                let f = Box::pin(async move {
                    let operation_result = previous_executing.await;
                    match operation_result {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            let factory_result = factory(&s, &e);
                            // if error operation fails saga will restart from last step
                            operation(factory_result)
                                .await
                                .map_err(WrappingError::from)?;
                            existing_saga.write().expect("existing saga").cancelled = true;
                            Err(e)
                        }
                    }
                });
                (current_state, f)
            }),
            persister: self.persister,
        }
    }

    pub fn lock_scope(&self) -> &LockScope {
        &self.lock_scope
    }
}

#[async_trait]
impl<State, FactoryData, OperationResult, WrappingError, Persister>
    SagaRunner<FactoryData, OperationResult, WrappingError>
    for SagaDefinition<State, FactoryData, OperationResult, WrappingError, Persister>
where
    FactoryData: Send,
    OperationResult: Send,
    WrappingError: From<PersistError> + Send,
    Persister: StepPersister + Clone + Send + Sync + 'static,
{
    async fn run(self, data: FactoryData) -> Result<OperationResult, WrappingError> {
        let lock_scope = self.lock_scope;
        self.persister
            .lock(lock_scope.clone(), LockType::Executing)
            .await
            .map_err(WrappingError::from)?;
        let saga_result = self.persister.retrieve(lock_scope.id).await;
        if let Ok(s) = saga_result {
            *self.existing_saga.write().expect("saga lock") = s;
        }

        let (_, f) = (self.operation)(data);
        let result = f.await;

        let finish = result.is_ok() || self.existing_saga.read().expect("saga lock").cancelled;

        self.persister
            .lock(
                lock_scope,
                if finish {
                    LockType::Finished
                } else {
                    LockType::Failed
                },
            )
            .await
            .map_err(WrappingError::from)?;
        result
    }

    async fn continue_from_last_step(self) -> Result<OperationResult, WrappingError>
    where
        FactoryData: DeserializeOwned,
    {
        let lock_scope = self.lock_scope;
        let saga = {
            self.persister
                .lock(lock_scope.clone(), LockType::Executing)
                .await
                .map_err(WrappingError::from)?;
            self.persister
                .retrieve(lock_scope.id)
                .await
                .map_err(WrappingError::from)?
        };

        let state = saga.states.get(&0).ok_or(PersistError::NotFound)?;
        let data = serde_json::from_str(state)
            .map_err(PersistError::from)
            .map_err(WrappingError::from)?;
        *self.existing_saga.write().expect("saga lock") = saga;

        let (_, f) = (self.operation)(data);
        let result = f.await;

        let finish = result.is_ok() || self.existing_saga.read().expect("saga lock").cancelled;

        self.persister
            .lock(
                lock_scope,
                if finish {
                    LockType::Finished
                } else {
                    LockType::Failed
                },
            )
            .await
            .map_err(WrappingError::from)?;
        result
    }
}

#[cfg(test)]
mod tests {
    use std::{fmt::Display, time::Duration};

    use uuid::Uuid;

    use crate::{
        persisters::{blackhole::Blackhole, in_memory::InMemoryPersister},
        {curry, curry2},
    };

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct DefinitionError(String);

    impl Display for DefinitionError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Error")
        }
    }
    impl From<PersistError> for DefinitionError {
        fn from(value: PersistError) -> Self {
            DefinitionError(value.to_string())
        }
    }
    impl Error for DefinitionError {}

    #[derive(Debug)]
    struct State {
        run_data: String,
        initial_data: u8,
    }

    impl State {
        fn new(run_data: String, initial_data: &u8) -> Self {
            assert_eq!(run_data, "run data");
            Self {
                run_data,
                initial_data: *initial_data,
            }
        }

        fn for_test1(&self, _initial_data: u8) -> usize {
            self.run_data.len() + self.initial_data as usize
        }

        fn for_test2(&self, test1_data: bool) -> String {
            test1_data.to_string()
        }

        fn for_test3(&self, test2_data: Option<char>) -> (String, bool) {
            (self.run_data.clone(), test2_data == Some('t'))
        }

        fn for_test4(&self, test3_data: (String, String)) -> String {
            test3_data.0 + &test3_data.1
        }

        fn handle_produce_error(&self, _e: &DefinitionError) -> bool {
            true
        }
    }

    struct Test1AndTest2 {}

    impl Test1AndTest2 {
        async fn test1(&self, v: usize) -> Result<bool, DefinitionError> {
            Ok(v > 10)
        }

        async fn test2(&self, v: String) -> Result<Option<char>, DefinitionError> {
            Ok(v.chars().next())
        }
    }

    struct Test3AndTest4 {
        success: bool,
    }

    impl Test3AndTest4 {
        async fn test3(
            &self,
            run_data: String,
            success: bool,
        ) -> Result<(String, String), DefinitionError> {
            if self.success || success {
                Ok((run_data, "test3".to_string()))
            } else {
                Err(DefinitionError("test3".to_string()))
            }
        }

        async fn test4(&self, out: String) -> Result<u32, DefinitionError> {
            Ok(out.len() as u32)
        }
    }

    async fn produce_error(success: &bool, v: usize) -> Result<bool, DefinitionError> {
        if *success {
            Ok(v > 10)
        } else {
            Err(DefinitionError("produce_error".to_string()))
        }
    }

    async fn stop_on_error(v: bool) -> Result<bool, DefinitionError> {
        Ok(v)
    }

    async fn test1(v: usize) -> Result<bool, DefinitionError> {
        Ok(v > 10)
    }

    async fn test2(v: String) -> Result<Option<char>, DefinitionError> {
        Ok(v.chars().next())
    }

    async fn test3(run_data: String, success: bool) -> Result<(String, String), DefinitionError> {
        if success {
            Ok((run_data, "test3".to_string()))
        } else {
            Err(DefinitionError("test3".to_string()))
        }
    }

    async fn test4(out: String) -> Result<u32, DefinitionError> {
        Ok(out.len() as u32)
    }

    fn create_definition1() -> SagaDefinition<State, String, u8, DefinitionError, Blackhole> {
        let lock_scope = LockScope::from_id(Uuid::new_v4(), "create_definition1".to_string());
        SagaDefinition::new(lock_scope, State::new, 0, Blackhole::default())
    }

    fn create_definition2(
    ) -> SagaDefinition<State, String, Option<char>, DefinitionError, Blackhole> {
        let lock_scope = LockScope::from_id(Uuid::new_v4(), "create_definition2".to_string());
        SagaDefinition::new(lock_scope, State::new, 3, Blackhole::default())
            .step(test1, State::for_test1)
            .step(test2, State::for_test2)
    }

    fn create_definition3<P: StepPersister>(
        definition_id: Uuid,
        initial_data: u8,
        success: bool,
        p: P,
    ) -> SagaDefinition<State, String, u32, DefinitionError, P> {
        let lock_scope = LockScope::from_id(definition_id, "create_definition3".to_string());
        SagaDefinition::new(lock_scope, State::new, initial_data, p)
            .step(test1, State::for_test1)
            .step(test2, State::for_test2)
            .step(move |(a, b)| test3(a, b || success), State::for_test3)
            .step(test4, State::for_test4)
    }

    fn create_definition4<P: StepPersister>(
        definition_id: Uuid,
        initial_data: u8,
        success: bool,
        p: P,
    ) -> SagaDefinition<State, String, u32, DefinitionError, P> {
        let lock_scope = LockScope::from_id(definition_id, "create_definition4".to_string());
        let t1_and_t2 = Arc::new(Test1AndTest2 {});
        // let t2 = t1_and_t2.clone();
        let t3_and_t4 = Arc::new(Test3AndTest4 { success });
        // let t4 = Arc::new(Test3AndTest4 {});
        SagaDefinition::new(lock_scope, State::new, initial_data, p)
            .step(
                // |p| async move { t1_and_t2.test1(p).await },
                curry!(Test1AndTest2::test1, t1_and_t2.clone()),
                State::for_test1,
            )
            .step(
                // |p| async move { t2.test2(p).await },
                curry!(Test1AndTest2::test2, t1_and_t2),
                State::for_test2,
            )
            .step(
                // move |(a, b)| async move { t3_and_t4.test3(a, b || success).await },
                curry2!(Test3AndTest4::test3, t3_and_t4.clone()),
                State::for_test3,
            )
            .step(
                // |p| async move { t4.test4(p).await },
                curry!(Test3AndTest4::test4, t3_and_t4),
                State::for_test4,
            )
    }

    fn create_definition_with_error(
        success: bool,
    ) -> SagaDefinition<State, String, Option<char>, DefinitionError, Blackhole> {
        let lock_scope = LockScope::from_id(Uuid::new_v4(), "create_definition2".to_string());
        SagaDefinition::new(lock_scope, State::new, 3, Blackhole::default())
            .step(curry!(produce_error, success), State::for_test1)
            .on_error(stop_on_error, State::handle_produce_error)
            .step(test2, State::for_test2)
    }

    #[tokio::test]
    async fn test_definition1() {
        let definition = create_definition1();
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await.unwrap();
        assert_eq!(0, result);
    }

    #[tokio::test]
    async fn test_definition2() {
        let definition = create_definition2();
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await.unwrap();
        assert_eq!(Some('t'), result);
    }

    #[tokio::test]
    async fn test_definition3() {
        let definition_id = Uuid::new_v4();
        let definition = create_definition3(definition_id, 1, false, Blackhole::default());
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await;
        assert_eq!(Err(DefinitionError("test3".to_string())), result);

        let definition = create_definition3(definition_id, 6, false, Blackhole::default());
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await.unwrap();
        assert_eq!(13, result);
    }

    #[tokio::test]
    async fn test_definition3_continue() {
        let definition_id = Uuid::new_v4();
        let persister = InMemoryPersister::new(Duration::from_millis(5));
        let definition = create_definition3(definition_id, 1, false, persister.clone());
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await;
        assert_eq!(Err(DefinitionError("test3".to_string())), result);

        let definition = create_definition3(definition_id, 6, true, persister);
        let result = definition.continue_from_last_step().await.unwrap();
        assert_eq!(13, result);
    }

    #[tokio::test]
    async fn test_definition3_continue_error() {
        let definition_id = Uuid::new_v4();
        let persister = InMemoryPersister::new(Duration::from_millis(5));
        let definition = create_definition3(definition_id, 6, true, persister);
        let result = definition.continue_from_last_step().await;
        assert!(matches!(result, Err(DefinitionError(_))));
    }

    #[tokio::test]
    async fn test_definition4_continue() {
        let definition_id = Uuid::new_v4();
        let persister = InMemoryPersister::new(Duration::from_millis(5));
        let definition = create_definition4(definition_id, 1, false, persister.clone());
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await;
        assert_eq!(Err(DefinitionError("test3".to_string())), result);

        let definition = create_definition4(definition_id, 6, true, persister);
        let result = definition.continue_from_last_step().await.unwrap();
        assert_eq!(13, result);
    }

    #[tokio::test]
    async fn test_definition_with_error() {
        let definition = create_definition_with_error(true);
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await.unwrap();
        assert_eq!(Some('t'), result);

        let definition = create_definition_with_error(false);
        let run_with_data = "run data".to_string();
        let result = definition.run(run_with_data.clone()).await;
        assert!(matches!(result, Err(DefinitionError(_))));
    }
}
