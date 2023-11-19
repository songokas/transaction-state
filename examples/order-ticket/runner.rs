use std::sync::{Arc, Mutex};

use transaction_state::persisters::persister::DefinitionPersister;
use uuid::Uuid;

use crate::{
    definitions::{existing_order::create_from_existing_order, full_order::create_full_order},
    models::{
        email::EmailId,
        error::{GeneralError, LocalError},
    },
};

pub async fn run_definition<P>(
    persister: Arc<Mutex<P>>,
    name: String,
    id: Uuid,
    executor_id: Uuid,
) -> Result<(), GeneralError>
where
    P: DefinitionPersister + Send + 'static,
{
    match name.as_str() {
        "create_from_existing_order" => {
            let c = create_from_existing_order(persister, id, true, executor_id);
            let r: Result<EmailId, GeneralError> = c.continue_from_last_step().await;
            println!("Retried email {r:?}");
            Ok(())
        }
        "create_full_order" => {
            let r: Result<EmailId, GeneralError> = {
                let a = create_full_order(persister, id, true, executor_id);
                a.continue_from_last_step().await
            };
            println!("Retried email {r:?}");
            Ok(())
        }
        _ => Err(LocalError {}.into()),
    }
}
