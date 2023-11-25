use std::{error::Error, fmt, time::Duration};

use rand::Rng;
use tokio::time::sleep;
use uuid::Uuid;

pub async fn execute_remote_service(id: Uuid) -> Result<Uuid, ExternalError> {
    let wait = rand::thread_rng().gen_range(32..89);
    let proceed = rand::thread_rng().gen_range(1..10) % 5 != 0;
    log::info!("execute_remote_service with {id} took {wait} {proceed}");
    sleep(Duration::from_millis(wait)).await;
    if proceed {
        Ok(Uuid::new_v4())
    } else {
        Err(ExternalError {})
    }
}

#[derive(Debug)]
pub struct ExternalError {}

impl fmt::Display for ExternalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExternalError")
    }
}

impl Error for ExternalError {}
