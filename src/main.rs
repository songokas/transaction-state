use std::{
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use uuid::Uuid;

type OperationDefinition<State, In, OperationResult, E> = Box<
    dyn FnOnce(
        In,
    ) -> (
        State,
        Pin<Box<dyn Future<Output = Result<OperationResult, E>>>>,
    ),
>;

struct SagaDefinition<State, In, OperationResult, E> {
    name: &'static str,
    operation: OperationDefinition<State, In, OperationResult, E>,
}

impl<State: Clone + 'static, FactoryData: 'static, OperationResult: 'static, WrappingError>
    SagaDefinition<State, FactoryData, OperationResult, WrappingError>
{
    fn new<StateCreator>(
        name: &'static str,
        state: StateCreator,
        initial_data: OperationResult,
    ) -> Self
    where
        StateCreator: FnOnce(FactoryData) -> State + 'static,
    {
        Self {
            name,
            operation: Box::new(|fd| {
                let s = state(fd);
                let f = Box::pin(async move { Result::<_, WrappingError>::Ok(initial_data) });
                (s, f)
            }),
        }
    }

    fn step<NewError, F, FactoryResult: 'static, Factory, Operation, NewFutureResult: 'static>(
        self,
        operation: Operation,
        factory: Factory,
    ) -> SagaDefinition<State, FactoryData, NewFutureResult, WrappingError>
    where
        Operation: FnOnce(FactoryResult) -> F + 'static,
        Factory: FnOnce(&State, OperationResult) -> FactoryResult + 'static,
        F: Future<Output = Result<NewFutureResult, NewError>> + 'static,
        WrappingError: Error + From<NewError> + 'static,
    {
        let previous = self.operation;
        SagaDefinition {
            name: self.name,
            operation: Box::new(|d| {
                let (prs, prf) = previous(d);
                let s = prs.clone();
                let f = Box::pin(async move {
                    let prd = prf.await;
                    let ns = factory(&s, prd?);
                    operation(ns).await.map_err(WrappingError::from)
                });
                (prs, f)
            }),
        }
    }

    async fn run(self, data: FactoryData) -> Result<OperationResult, WrappingError> {
        let (_state, f) = (self.operation)(data);
        f.await
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

#[derive(Clone)]
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

async fn confirm_ticket((mut order, ticket_id): (Order, TicketId)) -> Result<Order, LocalError> {
    println!("confirm_ticket with {ticket_id}");
    db_transaction(|_t| {
        order.ticket_id = ticket_id.into();
        Ok(order)
    })
    .await
    .map_err(|_| LocalError {})
}

async fn send_ticket(order: Order) -> Result<EmailId, ExternalError> {
    println!("send_ticket with order {}", order.order_id);
    execute_remote_service(order.order_id).await
}

async fn execute_remote_service(id: Uuid) -> Result<Uuid, ExternalError> {
    println!("execute_remote_service with {id}");
    Ok(Uuid::new_v4())
}

#[derive(Clone)]
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

fn create_from_existing_order() -> SagaDefinition<SagaOrderState, Order, TicketId, GeneralError> {
    SagaDefinition::new("create_from_ticket", SagaOrderState::new, ())
        .step(create_ticket, SagaOrderState::create_ticket)
        .step(confirm_ticket, SagaOrderState::confirm_ticket)
        .step(send_ticket, SagaOrderState::send_ticket)
}

fn create_full_order() -> SagaDefinition<SagaFullOrderState, Option<Order>, TicketId, GeneralError>
{
    SagaDefinition::new(
        "create_from_existing_order",
        SagaFullOrderState::new,
        OrderId::new_v4(),
    )
    .step(create_order, SagaFullOrderState::generate_order_id)
    .step(
        create_ticket,
        SagaFullOrderState::set_order_and_create_ticket,
    )
    .step(confirm_ticket, SagaFullOrderState::confirm_ticket)
    .step(send_ticket, SagaFullOrderState::send_ticket)
}

#[tokio::main]
async fn main() {
    let order_id = OrderId::new_v4();
    // operation will run as long as transaction completes even if the server crashes
    let definition = create_from_existing_order();
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
    let r: EmailId = definition.run(order).await.unwrap();
    println!("Received email {r}");

    // operations will run one after another, will not rerun in case of a crash
    let definition = create_full_order();
    let r: EmailId = definition.run(None).await.unwrap();
    println!("Received email {r}");

    // let order = Order {};
    // let ticket = 1;
    // // operations will run one after another, will rerun in case of a crash
    // let definition = create_multiple_steps_persist();
    // let r: bool = definition.run((order, ticket)).await;
}
