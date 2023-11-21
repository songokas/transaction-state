use transaction_state::curry2;
use transaction_state::{
    definitions::saga_definition::SagaDefinition,
    persisters::persister::{LockScope, StepPersister},
};
use uuid::Uuid;

use crate::{
    models::{error::DefinitionExecutionError, order::Order, ticket::TicketId},
    services::ticket::{create_ticket, send_ticket, TickeConfirmator},
    states::existing_order::SagaOrderState,
};

pub fn create_from_existing_order<P: StepPersister>(
    persister: P,
    order_id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaOrderState, Order, TicketId, DefinitionExecutionError, P> {
    let ticket_confirmator = TickeConfirmator { success };
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
    .step(
        curry2!(TickeConfirmator::confirm_ticket, ticket_confirmator),
        SagaOrderState::confirm_ticket,
    )
    .step(send_ticket, SagaOrderState::send_ticket)
}
