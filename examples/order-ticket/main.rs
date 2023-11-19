use std::time::Duration;

use definitions::{existing_order::create_from_existing_order, full_order::create_full_order};
use models::order::OrderId;
use resumer::run_resumer;
use services::db_transaction::TransactionError;
use tokio::spawn;
use transaction_state::persisters::{
    in_memory::InMemoryPersister, persister::InitialDataPersister,
};
use uuid::Uuid;

use crate::{
    models::{email::EmailId, error::GeneralError, order::Order},
    services::db_transaction::start_transaction,
};

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
    let persister = InMemoryPersister::new(Duration::from_secs(5));

    let runner = spawn(run_resumer(
        persister.clone(),
        Duration::from_secs(5),
        Duration::from_millis(100),
    ));

    for _ in 0..100 {
        let order_id = OrderId::new_v4();
        // operation will run as long as transaction completes even if the server crashes
        let definition =
            create_from_existing_order(persister.clone(), order_id, rand::random(), Uuid::new_v4());
        let lock_scope = definition.lock_scope.clone();

        // must complete
        let order = start_transaction(|_t| {
            let order = Order {
                order_id,
                ticket_id: None,
            };
            persister
                .save_initial_state(lock_scope, &order)
                .map_err(|_| TransactionError {})?;
            Ok(order)
        })
        .await
        .unwrap();

        spawn(async move {
            let r: Result<EmailId, GeneralError> = definition.run(order.clone()).await;
            println!("Received email {r:?}");
        });

        // operations will run one after another, will not rerun in case of a crash
        let definition = create_full_order(
            persister.clone(),
            OrderId::new_v4(),
            rand::random(),
            Uuid::new_v4(),
        );
        spawn(async move {
            let r: Result<EmailId, GeneralError> = definition.run(None).await;
            println!("Received email {r:?}");
        });
    }

    runner.await.unwrap();
}
