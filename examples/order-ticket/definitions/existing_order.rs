use sqlx::{Pool, Postgres};
use transaction_state::{curry, curry2};
use transaction_state::{
    definitions::saga_definition::SagaDefinition,
    persisters::persister::{LockScope, StepPersister},
};
use uuid::Uuid;

use crate::services::order::cancel_order;
use crate::{
    models::{error::DefinitionExecutionError, order::Order, ticket::TicketId},
    services::ticket::{create_ticket, send_ticket, TicketConfirmator},
    states::existing_order::SagaOrderState,
};

pub fn create_definition_for_existing_order<P: StepPersister>(
    pool: Pool<Postgres>,
    persister: P,
    order_id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaOrderState, Order, TicketId, DefinitionExecutionError, P> {
    let ticket_confirmator = TicketConfirmator {
        pool: pool.clone(),
        success,
    };
    SagaDefinition::new(
        LockScope {
            id: order_id,
            name: "create_from_existing_order".to_string(),
            executor_id,
        },
        SagaOrderState::new,
        (),
        persister,
    )
    .step(create_ticket, SagaOrderState::create_ticket)
    .on_error(
        curry!(cancel_order, pool.clone()),
        SagaOrderState::cancel_order,
    )
    .step(
        curry2!(TicketConfirmator::confirm_ticket, ticket_confirmator),
        SagaOrderState::confirm_ticket,
    )
    .step(curry!(send_ticket, pool), SagaOrderState::send_ticket)
}
