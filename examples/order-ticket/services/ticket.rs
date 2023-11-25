use sqlx::{Pool, Postgres};

use crate::{
    models::{
        email::EmailId,
        error::{LocalError, TransactionError},
        order::Order,
        ticket::TicketId,
    },
    services::remote::execute_remote_service,
};

use super::remote::ExternalError;

pub async fn create_ticket(order: Order) -> Result<TicketId, ExternalError> {
    log::info!("create_ticket with order {}", order.order_id);
    let result = execute_remote_service(order.order_id).await?;
    Ok(result)
}

pub async fn send_ticket(pool: &Pool<Postgres>, order: Order) -> Result<EmailId, LocalError> {
    let email_id = execute_remote_service(order.order_id)
        .await
        .map_err(LocalError::External)?;

    log::info!("send_ticket with order {}", order.order_id);

    sqlx::query("UPDATE order_ticket SET email_id = $1 WHERE id = $2")
        .bind(email_id)
        .bind(order.order_id)
        .execute(pool)
        .await
        .map(|_| email_id)
        .map_err(|e| LocalError::Transaction(TransactionError(e.to_string())))
}

pub struct TicketConfirmator {
    pub pool: Pool<Postgres>,
    pub success: bool,
}

impl TicketConfirmator {
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
                .map_err(|e| LocalError::Transaction(TransactionError(e.to_string())))?;

            log::info!("confirm_ticket {ticket_id} for order {}", order.order_id);

            order.ticket_id = ticket_id.into();
            Ok(order)
        } else {
            Err(LocalError::Other("Failing confirm_ticket".to_string()))
        }
    }
}
