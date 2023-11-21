use chrono::Utc;
use sqlx::{Pool, Postgres};

use crate::models::{
    error::LocalError,
    order::{Order, OrderId},
};

pub async fn create_order(pool: &Pool<Postgres>, order_id: OrderId) -> Result<Order, LocalError> {
    sqlx::query(
        "INSERT INTO order_ticket (id, dtc)
    VALUES ($1, $2)",
    )
    .bind(order_id)
    .bind(Utc::now().naive_utc())
    .execute(pool)
    .await
    .map_err(|_| LocalError {})?;

    println!("create_order with {order_id}");

    Ok(Order {
        order_id,
        ticket_id: None,
    })
}
