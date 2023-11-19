use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
};

use futures::future::BoxFuture;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{spawn, task::spawn_blocking};
use uuid::Uuid;

type OperationDefinition<State, In, OperationResult, E> = Box<
    dyn FnOnce(
        In,
    ) -> (
        State,
        Pin<Box<dyn Future<Output = Result<OperationResult, E>> + Send>>,
    ),
>;

#[derive(Debug, Clone)]
struct Saga {
    pub id: Uuid,
    pub states: BTreeMap<u8, String>,
}

impl Saga {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            states: Default::default(),
        }
    }
    pub fn last_step(&self) -> u8 {
        self.states.last_key_value().map(|(k, _)| *k).unwrap_or(0)
    }
}

trait DefinitionPersister {
    fn retrieve(&self, id: Uuid) -> Result<Saga, String>;
    fn store(&mut self, saga: Saga) -> Result<(), String>;
}

#[derive(Debug, Default)]
struct InMemoryPersister {
    sagas: HashMap<Uuid, Saga>,
}

impl DefinitionPersister for InMemoryPersister {
    fn retrieve(&self, id: Uuid) -> Result<Saga, String> {
        self.sagas
            .get(&id)
            .cloned()
            .ok_or_else(|| "Not found".to_string())
    }

    fn store(&mut self, saga: Saga) -> Result<(), String> {
        self.sagas.insert(saga.id, saga);
        Ok(())
    }
}

struct SagaDefinition<State, In, OperationResult, E, Persister> {
    name: &'static str,
    id: Uuid,
    step: u8,
    operation: OperationDefinition<Arc<State>, In, OperationResult, E>,
    persister: Arc<Mutex<Persister>>,
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
    fn new<StateCreator>(
        name: &'static str,
        id: Uuid,
        state: StateCreator,
        initial_data: OperationResult,
        persister: Arc<Mutex<Persister>>,
    ) -> Self
    where
        Persister: DefinitionPersister + Send,
        StateCreator: FnOnce(FactoryData) -> State + Send + 'static,
    {
        let step = 0;
        Self {
            name,
            id,
            step,
            existing_saga: Arc::new(RwLock::new(Saga::new(id))),
            operation: Box::new(|fd| {
                let s = state(fd);
                let f = Box::pin(async move { Result::<_, WrappingError>::Ok(initial_data) });
                (Arc::new(s), f)
            }),
            persister,
        }
    }

    fn step<
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
        WrappingError: Error + From<NewError> + From<serde_json::Error> + Send + 'static,
        NewFutureResult: Serialize + DeserializeOwned + Send,
        Persister: DefinitionPersister + Send + 'static,
    {
        let previous = self.operation;
        let persister = self.persister.clone();
        let definition_step = self.step + 1;
        let existing_saga = self.existing_saga.clone();
        SagaDefinition {
            name: self.name,
            id: self.id,
            step: definition_step,
            existing_saga: self.existing_saga.clone(),
            operation: Box::new(move |d| {
                let (current_state, previous_executing) = previous(d);

                let s = current_state.clone();
                let f = Box::pin(async move {
                    let operation_result = previous_executing.await?;
                    let existing_state = {
                        existing_saga
                            .read()
                            .unwrap()
                            .states
                            .get(&definition_step)
                            .map(|s| serde_json::from_str(s))
                    };
                    if let Some(new_operation_result) = existing_state {
                        new_operation_result.map_err(WrappingError::from)
                    } else {
                        let factory_result = factory(&s, operation_result);

                        let new_operation_result =
                            operation(factory_result).await.map_err(WrappingError::from);

                        if let Ok(r) = &new_operation_result {
                            let state = serde_json::to_string(r).unwrap();
                            let mut saga = existing_saga.write().unwrap();
                            saga.states.insert(definition_step, state);
                            persister.lock().unwrap().store(saga.clone()).unwrap();
                        }
                        new_operation_result
                    }
                });
                (current_state, f)
            }),
            persister: self.persister,
        }
    }

    async fn run(self, data: FactoryData) -> Result<OperationResult, WrappingError>
    where
        Persister: DefinitionPersister + Send,
        OperationResult: Send,
        WrappingError: Send,
    {
        if let Ok(saga) = self.persister.lock().unwrap().retrieve(self.id) {
            *self.existing_saga.write().unwrap() = saga;
        }
        let (_state, f) = (self.operation)(data);
        spawn(f).await.unwrap()
    }
}

#[derive(Debug)]
struct Transaction {}

#[derive(Debug)]
struct TransactionError {}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TransactionError")
    }
}

impl Error for TransactionError {}

#[derive(Debug)]
struct LocalError {}

impl fmt::Display for LocalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LocalError")
    }
}

impl Error for LocalError {}

#[derive(Debug)]
struct ExternalError {}

impl fmt::Display for ExternalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExternalError")
    }
}

impl Error for ExternalError {}

#[derive(Debug)]
struct ConversionError {}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ConversionError")
    }
}

impl Error for ConversionError {}

#[derive(Debug)]
enum GeneralError {
    LocalError(LocalError),
    ExternalError(ExternalError),
    SerializiationError(serde_json::Error),
}

impl From<LocalError> for GeneralError {
    fn from(value: LocalError) -> Self {
        GeneralError::LocalError(value)
    }
}

impl From<ExternalError> for GeneralError {
    fn from(value: ExternalError) -> Self {
        GeneralError::ExternalError(value)
    }
}

impl From<serde_json::Error> for GeneralError {
    fn from(value: serde_json::Error) -> Self {
        GeneralError::SerializiationError(value)
    }
}

impl fmt::Display for GeneralError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "GeneralError")
    }
}

impl Error for GeneralError {}

async fn db_transaction<T>(
    callback: impl FnOnce(Transaction) -> Result<T, TransactionError>,
) -> Result<T, TransactionError> {
    println!("db_transaction");
    callback(Transaction {})
}

// create order - local service
// create ticket - external service
// confirm ticket - local service
// send ticket by mail - external service

type OrderId = Uuid;
type TicketId = Uuid;
type EmailId = Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Order {
    order_id: OrderId,
    ticket_id: Option<TicketId>,
}

async fn create_order(order_id: OrderId) -> Result<Order, LocalError> {
    println!("create_order with {order_id}");
    db_transaction(|_t| {
        Ok(Order {
            order_id,
            ticket_id: None,
        })
    })
    .await
    .map_err(|_| LocalError {})
}

async fn create_ticket(order: Order) -> Result<TicketId, ExternalError> {
    println!("create_ticket with order {}", order.order_id);
    execute_remote_service(order.order_id).await
}

struct TickeConfirmator {
    success: bool,
}

impl TickeConfirmator {
    async fn confirm_ticket(
        &self,
        mut order: Order,
        ticket_id: TicketId,
    ) -> Result<Order, LocalError> {
        println!("confirm_ticket with {ticket_id}");
        if self.success {
            db_transaction(|_t| {
                order.ticket_id = ticket_id.into();
                Ok(order)
            })
            .await
            .map_err(|_| LocalError {})
        } else {
            Err(LocalError {})
        }
    }
}

async fn send_ticket(order: Order) -> Result<EmailId, ExternalError> {
    println!("send_ticket with order {}", order.order_id);
    execute_remote_service(order.order_id).await
}

async fn execute_remote_service(id: Uuid) -> Result<Uuid, ExternalError> {
    println!("execute_remote_service with {id}");
    Ok(Uuid::new_v4())
}

#[derive(Debug, Clone)]
struct SagaOrderState {
    order: Order,
}

impl SagaOrderState {
    fn new(order: Order) -> SagaOrderState {
        Self { order }
    }

    fn create_ticket(&self, _: ()) -> Order {
        self.order.clone()
    }

    fn confirm_ticket(&self, ticket_id: TicketId) -> (Order, TicketId) {
        (self.order.clone(), ticket_id)
    }

    fn send_ticket(&self, order: Order) -> Order {
        order
    }
}

#[derive(Clone)]
struct SagaFullOrderState {
    order: Arc<Mutex<Option<Order>>>,
}

impl SagaFullOrderState {
    fn new(_order: Option<Order>) -> Self {
        Self {
            order: Arc::new(Mutex::new(None)),
        }
    }

    fn generate_order_id(&self, order_id: OrderId) -> OrderId {
        order_id
    }

    fn set_order_and_create_ticket(&self, order: Order) -> Order {
        *self.order.lock().unwrap() = Some(order.clone());
        order
    }

    fn confirm_ticket(&self, ticket_id: TicketId) -> (Order, TicketId) {
        (self.order.lock().unwrap().clone().unwrap(), ticket_id)
    }

    fn send_ticket(&self, order: Order) -> Order {
        order
    }
}

fn create_from_existing_order<P>(
    persister: Arc<Mutex<P>>,
    order_id: Uuid,
    success: bool,
) -> SagaDefinition<SagaOrderState, Order, TicketId, GeneralError, P>
where
    P: DefinitionPersister + 'static + Send,
{
    let ticker_confirmator = TickeConfirmator { success };
    SagaDefinition::new(
        "create_from_ticket",
        order_id,
        SagaOrderState::new,
        (),
        persister,
    )
    .step(create_ticket, SagaOrderState::create_ticket)
    .step(
        |(o, t)| async move { ticker_confirmator.confirm_ticket(o, t).await },
        SagaOrderState::confirm_ticket,
    )
    .step(send_ticket, SagaOrderState::send_ticket)
}

fn create_full_order<P>(
    persister: Arc<Mutex<P>>,
    success: bool,
) -> SagaDefinition<SagaFullOrderState, Option<Order>, TicketId, GeneralError, P>
where
    P: DefinitionPersister + 'static + Send,
{
    let ticket_confirmator = TickeConfirmator { success };
    SagaDefinition::new(
        "create_from_existing_order",
        Uuid::new_v4(),
        SagaFullOrderState::new,
        OrderId::new_v4(),
        persister,
    )
    .step(create_order, SagaFullOrderState::generate_order_id)
    .step(
        create_ticket,
        SagaFullOrderState::set_order_and_create_ticket,
    )
    .step(
        |(o, t)| async move { ticket_confirmator.confirm_ticket(o, t).await },
        SagaFullOrderState::confirm_ticket,
    )
    .step(send_ticket, SagaFullOrderState::send_ticket)
}

#[tokio::main]
async fn main() {
    let persister = Arc::new(Mutex::new(InMemoryPersister::default()));
    let order_id = OrderId::new_v4();
    // operation will run as long as transaction completes even if the server crashes
    let definition = create_from_existing_order(persister.clone(), order_id, false);
    // must complete
    let order = db_transaction(|_t| {
        Ok(Order {
            order_id,
            ticket_id: None,
        })
        // definition.build(initial_data);
    })
    .await
    .unwrap();
    let r: Result<EmailId, GeneralError> = definition.run(order.clone()).await;
    println!("Received email {r:?}");

    let definition = create_from_existing_order(persister.clone(), order_id, true);
    let r: Result<EmailId, GeneralError> = definition.run(order).await;
    println!("Received email {r:?}");

    // operations will run one after another, will not rerun in case of a crash
    // let definition = create_full_order(persister.clone());
    // let r: EmailId = definition.run(None).await.unwrap();
    // println!("Received email {r}");

    // let order = Order {};
    // let ticket = 1;
    // // operations will run one after another, will rerun in case of a crash
    // let definition = create_multiple_steps_persist();
    // let r: bool = definition.run((order, ticket)).await;
}
