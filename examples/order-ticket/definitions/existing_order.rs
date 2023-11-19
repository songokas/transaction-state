use std::sync::{Arc, Mutex};

use transaction_state::{
    definitions::saga_definition::SagaDefinition,
    persisters::persister::{DefinitionPersister, LockScope},
};
use uuid::Uuid;

use crate::{
    models::{error::GeneralError, order::Order, ticket::TicketId},
    services::ticket::{create_ticket, send_ticket, TickeConfirmator},
    states::existing_order::SagaOrderState,
};

pub fn create_from_existing_order<P>(
    persister: Arc<Mutex<P>>,
    order_id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaOrderState, Order, TicketId, GeneralError, P>
where
    P: DefinitionPersister + 'static + Send,
{
    let ticker_confirmator = TickeConfirmator { success };
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
        |(o, t)| async move { ticker_confirmator.confirm_ticket(o, t).await },
        SagaOrderState::confirm_ticket,
    )
    .step(send_ticket, SagaOrderState::send_ticket)
}
