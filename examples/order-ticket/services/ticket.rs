use crate::{
    models::{email::EmailId, error::LocalError, order::Order, ticket::TicketId},
    services::{db_transaction::start_transaction, remote::execute_remote_service},
};

use super::remote::ExternalError;

pub async fn create_ticket(order: Order) -> Result<TicketId, ExternalError> {
    println!("create_ticket with order {}", order.order_id);
    execute_remote_service(order.order_id).await
}

pub async fn send_ticket(order: Order) -> Result<EmailId, ExternalError> {
    println!("send_ticket with order {}", order.order_id);
    execute_remote_service(order.order_id).await
}

pub struct TickeConfirmator {
    pub success: bool,
}

impl TickeConfirmator {
    pub async fn confirm_ticket(
        &self,
        mut order: Order,
        ticket_id: TicketId,
    ) -> Result<Order, LocalError> {
        println!("confirm_ticket with {ticket_id}");
        if self.success {
            start_transaction(|_t| {
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
