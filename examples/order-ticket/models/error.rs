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
pub enum GeneralError {
    LocalError(LocalError),
    ExternalError(ExternalError),
    SerializiationError(serde_json::Error),
    PersistError(PersistError),
}

impl From<LocalError> for GeneralError {
    fn from(value: LocalError) -> Self {
        GeneralError::LocalError(value)
    }
}

impl From<ExternalError> for GeneralError {
    fn from(value: ExternalError) -> Self {
        GeneralError::ExternalError(value)
    }
}

impl From<serde_json::Error> for GeneralError {
    fn from(value: serde_json::Error) -> Self {
        GeneralError::SerializiationError(value)
    }
}

impl From<PersistError> for GeneralError {
    fn from(value: PersistError) -> Self {
        GeneralError::PersistError(value)
    }
}

impl fmt::Display for GeneralError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "GeneralError")
    }
}

impl Error for GeneralError {}
