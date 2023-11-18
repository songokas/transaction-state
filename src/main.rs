use std::{error::Error, fmt, future::Future, marker::PhantomData, pin::Pin};

use uuid::Uuid;

fn combine<E, F, I: 'static, X, O: 'static, F1: 'static, F2: 'static>(
    f1: F1,
    f2: F2,
) -> impl FnOnce(I) -> Pin<Box<dyn Future<Output = Result<O, E>>>>
where
    F1: FnOnce(I) -> X,
    F2: FnOnce(X) -> F,
    F: Future<Output = Result<O, E>> + 'static,
    E: Error,
{
    move |i| Box::pin(f2(f1(i)))
}

fn combine_f<E, Fut1, Fut2, I: 'static, X, O, F1: 'static, F2: 'static>(
    f1: F1,
    f2: F2,
) -> impl FnOnce(I) -> Pin<Box<dyn Future<Output = Result<O, E>>>>
where
    F1: FnOnce(I) -> Fut1,
    F2: FnOnce(X) -> Fut2,
    Fut1: Future<Output = Result<X, E>> + 'static,
    Fut2: Future<Output = Result<O, E>> + 'static,
    E: Error,
{
    move |i| Box::pin(async { f2(f1(i).await?).await })
}

struct SagaDefinition<State, In, OperationResult, E> {
    name: &'static str,
    // state: State,
    operation:
        Box<dyn FnOnce(In) -> Pin<Box<dyn Future<Output = Result<OperationResult, E>>>> + 'static>,
    _state: PhantomData<State>,
}

impl<State: 'static, FactoryData: 'static, OperationResult: 'static, E: 'static>
    SagaDefinition<State, FactoryData, OperationResult, E>
{
    fn new<StateCreator, F, Operation, Factory, FactoryResult: 'static>(
        name: &'static str,
        state: StateCreator,
        operation: Operation,
        factory: Factory,
    ) -> Self
    where
        StateCreator: FnOnce(FactoryData) -> State + 'static,
        Factory: FnOnce(&State) -> FactoryResult + 'static,
        Operation: FnOnce(FactoryResult) -> F + 'static,
        F: Future<Output = Result<OperationResult, E>> + 'static,
        E: Error,
    {
        Self {
            name,
            // state,
            operation: Box::new(combine(state, combine(|state| factory(&state), operation))),
            _state: PhantomData::default(),
        }
    }

    // fn step<F, NewResult: 'static, Factory, Operation, NewFuture: 'static>(
    //     self,
    //     operation: Operation,
    //     factory: Factory,
    // ) -> SagaDefinition<State, FactoryData, NewFuture, E>
    // where
    //     Operation: FnOnce(NewResult) -> F + 'static,
    //     Factory: FnOnce(State, OperationResult) -> NewResult + 'static,
    //     F: Future<Output = NewFuture> + 'static,
    //     E: Error,
    // {
    //     SagaDefinition {
    //         name: self.name,
    //         // state: self.state,
    //         _state: PhantomData::default(),
    //         operation: Box::new(combine_f(self.operation, combine(factory, operation))),
    //     }
    // }

    async fn run(self, data: FactoryData) -> Result<OperationResult, E> {
        (self.operation)(data).await
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

#[derive(Clone, Copy)]
struct Order {
    order_id: OrderId,
    ticket_id: Option<TicketId>,
}

struct TicketEmail {
    email_id: EmailId,
}

async fn create_order(order_id: OrderId) -> Result<Order, LocalError> {
    println!("create_order with {order_id}");
    db_transaction(|t| {
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
    db_transaction(|t| {
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

struct SagaOrderState {
    order: Order,
}

impl SagaOrderState {
    fn new(order: Order) -> SagaOrderState {
        Self { order }
    }

    fn create_ticket(&self) -> Order {
        self.order
    }

    fn confirm_ticket(&self, ticket_id: TicketId) -> (Order, TicketId) {
        (self.order, ticket_id)
    }

    fn send_ticket(&self, _order: Order) -> Order {
        self.order
    }
}

fn create_from_ticket() -> SagaDefinition<SagaOrderState, Order, TicketId, ExternalError> {
    // let state = SagaOrderState { order: None };
    SagaDefinition::new(
        "create_from_ticket",
        SagaOrderState::new,
        create_ticket,
        SagaOrderState::create_ticket,
    )
    // .step(confirm_ticket, SagaOrderState::confirm_ticket)
    // .step(send_ticket, SagaOrderState::send_ticket)
}

// fn create_all() -> SagaDefinition {
//     SagaDefinition::new("create_multiple_steps", create_order, |data| data)
//         .step(create_ticket, convert_external_data)
//         .step(confirm_ticket)
//         .step(send_ticket)
// }

// fn create_all_persist() -> SagaDefinition {
//     SagaDefinition::new(
//         "create_multiple_steps_persist",
//         update_local_data,
//         |initial_data| initial_data,
//     )
//     .step(
//         |external_data| async {
//             // must complete
//             db_transaction(|_| {
//                 // update record
//                 // save external data
//             })
//             .await
//         },
//         convert_external_data,
//     )
// }

#[tokio::main]
async fn main() {
    let order_id = OrderId::new_v4();
    // operation will run as long as transaction completes even if the server crashes
    let definition = create_from_ticket();
    // must complete
    let order = db_transaction(|transaction| {
        Ok(Order {
            order_id,
            ticket_id: None,
        })
        // definition.build(initial_data);
    })
    .await
    .unwrap();
    let r: EmailId = definition.run(order).await.unwrap();

    // let order = Order {};
    // let ticket = 1;
    // // operations will run one after another, will not rerun in case of a crash
    // let definition = create_multiple_steps();
    // let r: bool = definition.run((order, ticket)).await;

    // let order = Order {};
    // let ticket = 1;
    // // operations will run one after another, will rerun in case of a crash
    // let definition = create_multiple_steps_persist();
    // let r: bool = definition.run((order, ticket)).await;
}
