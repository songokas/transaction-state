use std::collections::BTreeMap;

use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SagaState {
    pub id: Uuid,
    pub states: BTreeMap<u8, String>,
    pub cancelled: bool,
}

impl SagaState {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            states: Default::default(),
            cancelled: false,
        }
    }
    pub fn last_step(&self) -> u8 {
        self.states.last_key_value().map(|(k, _)| *k).unwrap_or(0)
    }
}
