use std::time::Duration;

use sqlx::{Pool, Postgres};
use tokio::{spawn, time::sleep};
use transaction_state::persisters::persister::StepPersister;

use crate::runner::run_definition;

pub async fn run_resumer<P: StepPersister + Clone + Send + 'static>(
    pool: Pool<Postgres>,
    persister: P,
    restart_with_duration: Duration,
    sleep_when_empty: Duration,
) -> u64 {
    let mut run_count = 0;
    let mut empty_count = 0;
    loop {
        let failed = {
            persister
                .get_next_failed(restart_with_duration)
                .await
                .ok()
                .flatten()
        };
        if let Some((id, name, executor_id)) = failed {
            empty_count = 0;
            run_count += 1;
            spawn(run_definition(
                pool.clone(),
                persister.clone(),
                name,
                id,
                executor_id,
            ));
        } else {
            empty_count += 1;
            sleep(sleep_when_empty).await;
        }

        if empty_count > 20 {
            break;
        }
    }
    run_count
}
