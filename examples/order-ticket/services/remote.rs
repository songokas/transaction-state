use std::{error::Error, fmt};

use uuid::Uuid;

pub async fn execute_remote_service(id: Uuid) -> Result<Uuid, ExternalError> {
    println!("execute_remote_service with {id}");
    Ok(Uuid::new_v4())
}

#[derive(Debug)]
pub struct ExternalError {}

impl fmt::Display for ExternalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExternalError")
    }
}

impl Error for ExternalError {}
