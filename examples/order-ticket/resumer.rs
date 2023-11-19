use std::time::Duration;

use tokio::{spawn, time::sleep};
use transaction_state::persisters::persister::DefinitionPersister;

use crate::runner::run_definition;

pub async fn run_resumer<P: DefinitionPersister + Clone + Send + 'static>(
    persister: P,
    restart_with_duration: Duration,
    sleep_when_empty: Duration,
) {
    loop {
        let failed = {
            persister
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
