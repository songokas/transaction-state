use std::time::Duration;

use definitions::{
    existing_order::create_definition_for_existing_order, full_order::create_full_order,
};
use env_logger::Env;
use models::order::OrderId;
use resumer::run_resumer;
use services::{
    order::create_order,
    sqlx_persister::{save_initial_state, SqlxPersister},
};
use sqlx::postgres::PgPoolOptions;
use tokio::spawn;
use transaction_state::definitions::saga_definition::SagaRunner;
use uuid::Uuid;

use crate::models::{email::EmailId, error::DefinitionExecutionError};

mod definitions;
mod models;
mod resumer;
mod runner;
mod services;
mod states;
mod transaction;

// create order - local service
// create ticket - external service
// confirm ticket - local service
// send ticket by mail - external service
#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let pool = PgPoolOptions::new()
        .max_connections(50)
        .connect_lazy("postgres://postgres:123456@localhost/order_ticket")
        .unwrap();

    let persister = SqlxPersister::new(pool.clone(), Duration::from_secs(10));
    // let persister = InMemoryPersister::new(Duration::from_secs(10));

    let runner = spawn(run_resumer(
        pool.clone(),
        persister.clone(),
        Duration::from_secs(10),
        Duration::from_millis(600),
    ));

    let mut orders = Vec::new();

    for _ in 0..100 {
        let order_id = OrderId::new_v4();
        // operation will run as long as transaction completes even if the server crashes
        let definition = create_definition_for_existing_order(
            pool.clone(),
            persister.clone(),
            order_id,
            rand::random(),
            Uuid::new_v4(),
        );
        let lock_scope = definition.lock_scope().clone();

        let order = {
            let mut tx = pool.begin().await.unwrap();
            let order = create_order(&mut tx, order_id).await.unwrap();

            save_initial_state(&mut tx, lock_scope, &order, Duration::from_secs(5))
                .await
                .unwrap();

            tx.commit().await.unwrap();
            order
        };

        orders.push(order.order_id);

        spawn(async move {
            let r: Result<EmailId, DefinitionExecutionError> = definition.run(order.clone()).await;
            log::info!("Received email {r:?}");
        });

        let order_id = OrderId::new_v4();
        orders.push(order_id);
        let definition = create_full_order(
            pool.clone(),
            persister.clone(),
            order_id,
            rand::random(),
            Uuid::new_v4(),
        );
        spawn(async move {
            let r: Result<EmailId, DefinitionExecutionError> = definition.run(None).await;
            log::info!("Received email {r:?}");
        });
    }

    let definitions_retried = runner.await.unwrap();

    let order_count = orders.len();

    let query = format!(
        "SELECT COUNT(ticket_id), COUNT(email_id), COUNT(cancelled) FROM order_ticket WHERE id IN({})",
        orders
            .iter()
            .map(|x| format!("'{x}'"))
            .collect::<Vec<_>>()
            .join(",")
    );

    let stat: (i64, i64, i64) = sqlx::query_as(&query).fetch_one(&pool).await.unwrap();

    log::info!(
        "Orders created: {order_count} Orders cancelled: {} Tickets updated: {} Emails sent: {} Definitions Retried: {definitions_retried}",
        stat.2,
        stat.0,
        stat.1,
    );
}
