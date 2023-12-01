use sqlx::{Pool, Postgres};
use transaction_state::{
    definitions::saga_definition::SagaRunner, persisters::persister::StepPersister,
};
use uuid::Uuid;

use crate::{
    definitions::{
        existing_order::create_definition_for_existing_order, full_order::create_full_order,
    },
    models::{email::EmailId, error::DefinitionExecutionError},
};

pub async fn run_definition<P: StepPersister + Clone + Send + 'static>(
    pool: Pool<Postgres>,
    persister: P,
    name: String,
    id: Uuid,
    executor_id: Uuid,
) -> Result<(), DefinitionExecutionError> {
    match name.as_str() {
        "create_from_existing_order" => {
            let c = create_definition_for_existing_order(pool, persister, id, true, executor_id);
            let r: Result<EmailId, DefinitionExecutionError> = c.continue_from_last_step().await;
            log::info!("Retried email {name} {id} {r:?}");
            Ok(())
        }
        "create_full_order" => {
            let r: Result<EmailId, DefinitionExecutionError> = {
                let a = create_full_order(pool, persister, id, true, executor_id);
                a.continue_from_last_step().await
            };
            log::info!("Retried email {name} {id} {r:?}");
            Ok(())
        }
        _ => panic!("unknown definition {name}"),
    }
}
