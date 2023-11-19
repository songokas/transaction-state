use std::{error::Error, fmt, time::Duration};

use serde::Serialize;
use uuid::Uuid;

use crate::definitions::saga::Saga;

pub trait DefinitionPersister {
    fn lock(&self, scope: LockScope, lock_type: LockType) -> Result<(), PersistError>;
    fn retrieve(&self, id: Uuid) -> Result<Saga, PersistError>;
    fn store(&self, saga: Saga) -> Result<(), PersistError>;
    fn get_next_failed(
        &self,
        for_duration: Duration,
    ) -> Result<Option<(Uuid, String, Uuid)>, PersistError>;
}

pub trait InitialDataPersister {
    fn save_initial_state<S: Serialize>(
        &self,
        scope: LockScope,
        state: &S,
    ) -> Result<(), PersistError>;
}

#[derive(Clone)]
pub struct LockScope {
    pub id: Uuid,
    pub executor_id: Uuid,
    pub name: String,
}

impl LockScope {
    pub fn from_id(id: Uuid, name: String) -> Self {
        Self {
            id,
            executor_id: Uuid::new_v4(),
            name,
        }
    }
}

#[derive(Debug)]
pub enum LockType {
    Executing,
    Failed,
    Finished,
    Initial,
    Retry,
}

#[derive(Debug)]
pub enum PersistError {
    Locked,
    NotFound,
    Serialization(serde_json::Error),
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Locked => write!(f, "Failed to persist: Locked"),
            Self::NotFound => write!(f, "Failed to persist: NotFound"),
            Self::Serialization(e) => write!(f, "Failed to serialize: {e}"),
        }
    }
}

impl From<serde_json::Error> for PersistError {
    fn from(value: serde_json::Error) -> Self {
        PersistError::Serialization(value)
    }
}

impl Error for PersistError {}
