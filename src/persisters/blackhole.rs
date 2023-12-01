use std::time::Duration;

use uuid::Uuid;

use crate::definitions::saga_state::SagaState;

use super::persister::{LockScope, LockType, PersistError, StepPersister};

#[derive(Default, Clone)]
pub struct Blackhole {}

#[async_trait::async_trait]
impl StepPersister for Blackhole {
    async fn lock(&self, _scope: LockScope, _lock_type: LockType) -> Result<(), PersistError> {
        Ok(())
    }

    async fn retrieve(&self, _id: Uuid) -> Result<SagaState, PersistError> {
        Err(PersistError::NotFound)
    }

    async fn store(&self, _id: Uuid, _step: u8, _state: String) -> Result<(), PersistError> {
        Ok(())
    }

    async fn get_next_failed(
        &self,
        _for_duration: Duration,
    ) -> Result<Option<(Uuid, String, Uuid)>, PersistError> {
        Err(PersistError::NotFound)
    }
}
