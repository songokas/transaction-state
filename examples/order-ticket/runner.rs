use transaction_state::persisters::persister::StepPersister;
use uuid::Uuid;

use crate::{
    definitions::{existing_order::create_from_existing_order, full_order::create_full_order},
    models::{
        email::EmailId,
        error::{DefinitionExecutionError, LocalError},
    },
};

pub async fn run_definition<P: StepPersister + Clone + Send + 'static>(
    persister: P,
    name: String,
    id: Uuid,
    executor_id: Uuid,
) -> Result<(), DefinitionExecutionError> {
    match name.as_str() {
        "create_from_existing_order" => {
            let c = create_from_existing_order(persister, id, true, executor_id);
            let r: Result<EmailId, DefinitionExecutionError> = c.continue_from_last_step().await;
            println!("Retried email {name} {id} {r:?}");
            Ok(())
        }
        "create_full_order" => {
            let r: Result<EmailId, DefinitionExecutionError> = {
                let a = create_full_order(persister, id, true, executor_id);
                a.continue_from_last_step().await
            };
            println!("Retried email {name} {id} {r:?}");
            Ok(())
        }
        _ => Err(LocalError {}.into()),
    }
}
