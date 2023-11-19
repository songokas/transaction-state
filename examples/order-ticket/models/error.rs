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
    Local(LocalError),
    External(ExternalError),
    Serializiation(serde_json::Error),
    Persist(PersistError),
}

impl From<LocalError> for GeneralError {
    fn from(value: LocalError) -> Self {
        GeneralError::Local(value)
    }
}

impl From<ExternalError> for GeneralError {
    fn from(value: ExternalError) -> Self {
        GeneralError::External(value)
    }
}

impl From<serde_json::Error> for GeneralError {
    fn from(value: serde_json::Error) -> Self {
        GeneralError::Serializiation(value)
    }
}

impl From<PersistError> for GeneralError {
    fn from(value: PersistError) -> Self {
        GeneralError::Persist(value)
    }
}

impl fmt::Display for GeneralError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GeneralError::External(e) => write!(f, "{e}"),
            GeneralError::Local(e) => write!(f, "{e}"),
            GeneralError::Serializiation(e) => write!(f, "{e}"),
            GeneralError::Persist(e) => write!(f, "{e}"),
        }
    }
}

impl Error for GeneralError {}
