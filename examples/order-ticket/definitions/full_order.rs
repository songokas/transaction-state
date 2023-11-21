use sqlx::{Pool, Postgres};
use transaction_state::{
    curry, curry2,
    definitions::saga_definition::SagaDefinition,
    persisters::persister::{LockScope, StepPersister},
};
use uuid::Uuid;

use crate::{
    models::{
        error::DefinitionExecutionError,
        order::{Order, OrderId},
        ticket::TicketId,
    },
    services::{
        order::create_order,
        ticket::{create_ticket, send_ticket, TickeConfirmator},
    },
    states::full_order::SagaFullOrderState,
};

pub fn create_full_order<P: StepPersister>(
    pool: Pool<Postgres>,
    persister: P,
    id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaFullOrderState, Option<Order>, TicketId, DefinitionExecutionError, P> {
    let ticket_confirmator = TickeConfirmator {
        pool: pool.clone(),
        success,
    };
    SagaDefinition::new(
        LockScope {
            id,
            name: "create_full_order".to_string(),
            executor_id,
        },
        SagaFullOrderState::new,
        OrderId::new_v4(),
        persister,
    )
    .step(
        curry!(create_order, pool),
        SagaFullOrderState::generate_order_id,
    )
    .step(
        create_ticket,
        SagaFullOrderState::set_order_and_create_ticket,
    )
    .step(
        curry2!(TickeConfirmator::confirm_ticket, ticket_confirmator),
        SagaFullOrderState::confirm_ticket,
    )
    .step(send_ticket, SagaFullOrderState::send_ticket)
}
