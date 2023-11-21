use std::time::Duration;

use chrono::Utc;
use definitions::{existing_order::create_from_existing_order, full_order::create_full_order};
use models::order::OrderId;
use resumer::run_resumer;
use services::sqlx_persister::SqlxPersister;
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

    // sqlx::query(
    //     "
    // CREATE TYPE lock_type AS ENUM ('Executing', 'Failed', 'Finished', 'Initial', 'Retry');
    // CREATE TABLE IF NOT EXISTS saga_step (
    //     id uuid NOT NULL,
    //     step smallint NOT NULL,
    //     state text NOT NULL,
    //     dtc TIMESTAMP NOT NULL DEFAULT NOW ()
    // );
    // CREATE TABLE IF NOT EXISTS saga_lock (
    //     id uuid NOT NULL,
    //     executor_id uuid NOT NULL,
    //     name varchar NOT NULL,
    //     lock lock_type NOT NULL,
    //     dtc TIMESTAMP NOT NULL DEFAULT NOW ()
    // );
    // CREATE TABLE IF NOT EXISTS order_ticket (
    //     id uuid PRIMARY KEY,
    //     ticket_id uuid NULL,
    //     dtc TIMESTAMP NOT NULL DEFAULT NOW ()
    // );

    // CREATE UNIQUE INDEX saga_step_id_step_idx ON saga_step (id, step);
    // CREATE INDEX saga_lock_id_idx ON saga_lock (id);
    // CREATE INDEX saga_lock_lock_idx ON saga_lock (lock);
    // CREATE INDEX saga_lock_dtc_idx ON saga_lock (dtc);
    // ",
    // )
    // .execute(&pool)
    // .await
    // .unwrap();

    let persister = SqlxPersister::new(pool.clone(), Duration::from_secs(5));
    // let persister = InMemoryPersister::new(Duration::from_secs(5));

    let runner = spawn(run_resumer(
        persister.clone(),
        Duration::from_secs(5),
        Duration::from_millis(600),
    ));

    for _ in 0..1 {
        let order_id = OrderId::new_v4();
        // operation will run as long as transaction completes even if the server crashes
        let definition =
            create_from_existing_order(persister.clone(), order_id, rand::random(), Uuid::new_v4());
        let lock_scope = definition.lock_scope.clone();
        let mut tx = pool.begin().await.unwrap();
        // let order = start_transaction(|_t| {
        let order = Order {
            order_id,
            ticket_id: None,
        };

        sqlx::query(
            "INSERT INTO order_ticket (id, dtc)
            VALUES ($1, $2)",
        )
        .bind(order.order_id)
        .bind(Utc::now().naive_utc())
        .execute(&mut *tx)
        .await
        .unwrap();

        // persister
        //     .save_initial_state(lock_scope, &order)
        //     .map_err(|_| TransactionError {})
        //     .await
        //     .unwrap();
        // Ok(order)
        // })
        // .await
        // .unwrap();

        tx.commit().await.unwrap();

        spawn(async move {
            let r: Result<EmailId, DefinitionExecutionError> = definition.run(order.clone()).await;
            println!("Received email {r:?}");
        });

        let definition = create_full_order(
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
