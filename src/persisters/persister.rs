use std::{error::Error, fmt, time::Duration};

use serde::Serialize;
use uuid::Uuid;

use crate::definitions::saga::Saga;

pub trait DefinitionPersister {
    fn lock(&mut self, scope: LockScope, lock_type: LockType) -> Result<(), PersistError>;
    fn retrieve(&self, id: Uuid) -> Result<Saga, PersistError>;
    fn store(&mut self, saga: Saga) -> Result<(), PersistError>;
    fn get_next_failed(
        &mut self,
        for_duration: Duration,
    ) -> Result<Option<(Uuid, String, Uuid)>, PersistError>;
    fn save_initial_state<S: Serialize>(
        &mut self,
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
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Locked => write!(f, "Failed to persist: Locked"),
            Self::NotFound => write!(f, "Failed to persist: NotFound"),
        }
    }
}

impl Error for PersistError {}
