use std::{error::Error, fmt};

pub async fn start_transaction<T>(
    callback: impl FnOnce(Connection) -> Result<T, TransactionError>,
) -> Result<T, TransactionError> {
    println!("db_transaction");
    callback(Connection {})
}

#[derive(Debug)]
pub struct Connection {}

#[derive(Debug)]
pub struct TransactionError {}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TransactionError")
    }
}

impl Error for TransactionError {}
