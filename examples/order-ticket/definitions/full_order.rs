use transaction_state::{
    definitions::saga_definition::SagaDefinition,
    persisters::persister::{DefinitionPersister, LockScope},
};
use uuid::Uuid;

use crate::{
    models::{
        error::GeneralError,
        order::{Order, OrderId},
        ticket::TicketId,
    },
    services::{
        order::create_order,
        ticket::{create_ticket, send_ticket, TickeConfirmator},
    },
    states::full_order::SagaFullOrderState,
};

pub fn create_full_order<P: DefinitionPersister + Clone + Send + 'static>(
    persister: P,
    id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaFullOrderState, Option<Order>, TicketId, GeneralError, P> {
    let ticket_confirmator = TickeConfirmator { success };
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
