use crate::{
    models::{
        error::LocalError,
        order::{Order, OrderId},
    },
    services::db_transaction::start_transaction,
};

pub async fn create_order(order_id: OrderId) -> Result<Order, LocalError> {
    println!("create_order with {order_id}");
    start_transaction(|_t| {
        Ok(Order {
            order_id,
            ticket_id: None,
        })
    })
    .await
    .map_err(|_| LocalError {})
}
