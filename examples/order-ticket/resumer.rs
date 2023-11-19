use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{spawn, time::sleep};
use transaction_state::persisters::persister::DefinitionPersister;

use crate::runner::run_definition;

pub async fn run_resumer<P>(
    persister: Arc<Mutex<P>>,
    restart_with_duration: Duration,
    sleep_when_empty: Duration,
) where
    P: DefinitionPersister + std::fmt::Debug + Send + 'static,
{
    loop {
        let failed = {
            persister
                .lock()
                .expect("persister lock")
                .get_next_failed(restart_with_duration)
                .ok()
                .flatten()
        };
        if let Some((id, name, executor_id)) = failed {
            spawn(run_definition(persister.clone(), name, id, executor_id));
        }
        sleep(sleep_when_empty).await;
    }
}
