use std::{error::Error, fmt, time::Duration};

use uuid::Uuid;

use crate::definitions::saga::Saga;

#[async_trait::async_trait]
pub trait StepPersister: Clone + Send + Sync + 'static {
    async fn lock(&self, scope: LockScope, lock_type: LockType) -> Result<(), PersistError>;
    async fn retrieve(&self, id: Uuid) -> Result<Saga, PersistError>;
    async fn store(&self, id: Uuid, step: u8, state: String) -> Result<(), PersistError>;
    async fn get_next_failed(
        &self,
        for_duration: Duration,
    ) -> Result<Option<(Uuid, String, Uuid)>, PersistError>;
}

// persist data before running while in transaction
// #[async_trait::async_trait]
// pub trait InitialDataPersister<T> {
//     async fn save_initial_state(
//         &self,
//         transaction: T,
//         scope: LockScope,
//     ) -> Result<(), PersistError>;
// }

// #[async_trait::async_trait]
// pub trait InitialDataPersister {
//     async fn save_initial_state<T, S: Serialize + Send + Sync>(
//         &self,
//         transaction: T,
//         scope: LockScope,
//         state: &S,
//     ) -> Result<(), PersistError>;
// }

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
    Execution(String, String),
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Locked => write!(f, "Record is locked"),
            Self::NotFound => write!(f, "Record not found"),
            Self::Serialization(e) => write!(f, "Failed to serialize: {e}"),
            Self::Execution(e, c) => write!(f, "Failed to execute {c}: {e}"),
        }
    }
}

impl From<serde_json::Error> for PersistError {
    fn from(value: serde_json::Error) -> Self {
        PersistError::Serialization(value)
    }
}

impl Error for PersistError {}
