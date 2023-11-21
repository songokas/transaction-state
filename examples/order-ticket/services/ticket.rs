use sqlx::{Pool, Postgres};

use crate::{
    models::{email::EmailId, error::LocalError, order::Order, ticket::TicketId},
    services::remote::execute_remote_service,
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
    pub pool: Pool<Postgres>,
    pub success: bool,
}

impl TickeConfirmator {
    pub async fn confirm_ticket(
        &self,
        mut order: Order,
        ticket_id: TicketId,
    ) -> Result<Order, LocalError> {
        if self.success {
            sqlx::query("UPDATE order_ticket SET ticket_id = $1 WHERE id = $2")
                .bind(ticket_id)
                .bind(order.order_id)
                .execute(&self.pool)
                .await
                .map_err(|_| LocalError {})?;

            println!("confirm_ticket {ticket_id} for order {}", order.order_id);

            order.ticket_id = ticket_id.into();
            Ok(order)
        } else {
            Err(LocalError {})
        }
    }
}
