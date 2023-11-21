use std::{error::Error, fmt};

use transaction_state::persisters::persister::PersistError;

use crate::services::remote::ExternalError;

#[derive(Debug)]
pub struct LocalError {}

impl fmt::Display for LocalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LocalError")
    }
}

impl Error for LocalError {}

#[derive(Debug)]
pub enum DefinitionExecutionError {
    Local(LocalError),
    External(ExternalError),
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

impl fmt::Display for DefinitionExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DefinitionExecutionError::External(e) => write!(f, "Definition execution failed: {e}"),
            DefinitionExecutionError::Local(e) => write!(f, "Definition execution failed: {e}"),
            DefinitionExecutionError::Persist(e) => write!(f, "Definition execution failed: {e}"),
        }
    }
}

impl Error for DefinitionExecutionError {}
