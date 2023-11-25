use std::{error::Error, fmt};

use transaction_state::persisters::persister::PersistError;

use crate::services::remote::ExternalError;

#[derive(Debug)]
pub enum LocalError {
    External(ExternalError),
    Transaction(TransactionError),
    Other(String),
}

impl fmt::Display for LocalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LocalError::External(e) => write!(f, "LocalError: {e}"),
            LocalError::Transaction(e) => write!(f, "LocalError: {e}"),
            LocalError::Other(e) => write!(f, "LocalError: {e}"),
        }
    }
}

impl Error for LocalError {}

#[derive(Debug)]
pub enum DefinitionExecutionError {
    Local(LocalError),
    External(ExternalError),
    Transaction(TransactionError),
    Persist(PersistError),
}

impl From<LocalError> for DefinitionExecutionError {
    fn from(value: LocalError) -> Self {
        DefinitionExecutionError::Local(value)
    }
}

impl From<ExternalError> for DefinitionExecutionError {
    fn from(value: ExternalError) -> Self {
        DefinitionExecutionError::External(value)
    }
}

impl From<PersistError> for DefinitionExecutionError {
    fn from(value: PersistError) -> Self {
        DefinitionExecutionError::Persist(value)
    }
}

impl From<TransactionError> for DefinitionExecutionError {
    fn from(value: TransactionError) -> Self {
        DefinitionExecutionError::Transaction(value)
    }
}

impl fmt::Display for DefinitionExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DefinitionExecutionError::External(e) => write!(f, "Definition execution failed: {e}"),
            DefinitionExecutionError::Local(e) => write!(f, "Definition execution failed: {e}"),
            DefinitionExecutionError::Persist(e) => write!(f, "Definition execution failed: {e}"),
            DefinitionExecutionError::Transaction(e) => {
                write!(f, "Definition execution failed: {e}")
            }
        }
    }
}

impl Error for DefinitionExecutionError {}

#[derive(Debug)]
pub struct TransactionError(pub String);

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TransactionError: {}", self.0)
    }
}

impl Error for TransactionError {}
