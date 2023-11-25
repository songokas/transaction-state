use sqlx::{Pool, Postgres};
use transaction_state::{
    curry, curry2, curry_inner,
    definitions::saga_definition::SagaDefinition,
    persisters::persister::{LockScope, StepPersister},
};
use uuid::Uuid;

use crate::{
    models::{error::DefinitionExecutionError, order::Order, ticket::TicketId},
    services::{
        order::{cancel_order, create_order},
        ticket::{create_ticket, send_ticket, TicketConfirmator},
    },
    states::full_order::SagaFullOrderState,
    transaction::execute_transaction,
};

pub fn create_full_order<P: StepPersister>(
    pool: Pool<Postgres>,
    persister: P,
    id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaFullOrderState, Option<Order>, TicketId, DefinitionExecutionError, P> {
    let ticket_confirmator = TicketConfirmator {
        pool: pool.clone(),
        success,
    };
    // let cpool = pool.clone();
    SagaDefinition::new(
        LockScope {
            id,
            name: "create_full_order".to_string(),
            executor_id,
        },
        SagaFullOrderState::new,
        id,
        persister,
    )
    .step(
        curry_inner!(execute_transaction, pool.clone(), create_order),
        SagaFullOrderState::generate_order_id,
    )
    .step(
        create_ticket,
        SagaFullOrderState::set_order_and_create_ticket,
    )
    .on_error(
        curry!(cancel_order, pool.clone()),
        SagaFullOrderState::cancel_order,
    )
    .step(
        curry2!(TicketConfirmator::confirm_ticket, ticket_confirmator),
        SagaFullOrderState::confirm_ticket,
    )
    .step(curry!(send_ticket, pool), SagaFullOrderState::send_ticket)
}
