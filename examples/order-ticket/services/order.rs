use chrono::Utc;
use sqlx::{Pool, Postgres, Transaction};

use crate::models::{
    error::{LocalError, TransactionError},
    order::{Order, OrderId},
};

pub async fn create_order(
    tx: &mut Transaction<'_, Postgres>,
    order_id: OrderId,
) -> Result<Order, LocalError> {
    sqlx::query(
        "INSERT INTO order_ticket (id, dtc)
    VALUES ($1, $2)",
    )
    .bind(order_id)
    .bind(Utc::now().naive_utc())
    .execute(&mut **tx)
    .await
    .map_err(|e| LocalError::Transaction(TransactionError(e.to_string())))?;

    log::info!("create_order with {order_id}");

    Ok(Order {
        order_id,
        ticket_id: None,
    })
}

pub async fn cancel_order(pool: &Pool<Postgres>, id: OrderId) -> Result<bool, TransactionError> {
    log::info!("cancel_order with order_id {}", id);

    sqlx::query("UPDATE order_ticket SET cancelled = $1 WHERE id = $2")
        .bind(Utc::now().naive_utc())
        .bind(id)
        .execute(pool)
        .await
        .map(|_| true)
        .map_err(|e| TransactionError(e.to_string()))
}
