use std::time::Duration;

use chrono::Utc;
use definitions::{existing_order::create_from_existing_order, full_order::create_full_order};
use models::order::OrderId;
use resumer::run_resumer;
use services::sqlx_persister::{save_initial_state, SqlxPersister};
use sqlx::postgres::PgPoolOptions;
use tokio::spawn;
use uuid::Uuid;

use crate::models::{email::EmailId, error::DefinitionExecutionError, order::Order};

mod definitions;
mod models;
mod resumer;
mod runner;
mod services;
mod states;

// create order - local service
// create ticket - external service
// confirm ticket - local service
// send ticket by mail - external service
#[tokio::main]
async fn main() {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:123456@localhost/order_ticket")
        .await
        .unwrap();
    sqlx::migrate!("./examples/order-ticket/migrations")
        .run(&pool)
        .await
        .unwrap();

    let persister = SqlxPersister::new(pool.clone(), Duration::from_secs(5));

    let runner = spawn(run_resumer(
        pool.clone(),
        persister.clone(),
        Duration::from_secs(5),
        Duration::from_millis(600),
    ));

    for _ in 0..100 {
        let order_id = OrderId::new_v4();
        // operation will run as long as transaction completes even if the server crashes
        let definition = create_from_existing_order(
            pool.clone(),
            persister.clone(),
            order_id,
            rand::random(),
            Uuid::new_v4(),
        );
        let lock_scope = definition.lock_scope.clone();
        let order = Order {
            order_id,
            ticket_id: None,
        };
        {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query(
                "INSERT INTO order_ticket (id, dtc)
            VALUES ($1, $2)",
            )
            .bind(order.order_id)
            .bind(Utc::now().naive_utc())
            .execute(&mut *tx)
            .await
            .unwrap();

            save_initial_state(&mut tx, lock_scope, &order, Duration::from_secs(5))
                .await
                .unwrap();

            tx.commit().await.unwrap();
        }

        spawn(async move {
            let r: Result<EmailId, DefinitionExecutionError> = definition.run(order.clone()).await;
            println!("Received email {r:?}");
        });

        let definition = create_full_order(
            pool.clone(),
            persister.clone(),
            OrderId::new_v4(),
            rand::random(),
            Uuid::new_v4(),
        );
        spawn(async move {
            let r: Result<EmailId, DefinitionExecutionError> = definition.run(None).await;
            println!("Received email {r:?}");
        });
    }

    runner.await.unwrap();
}
