use std::collections::BTreeMap;

use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Saga {
    pub id: Uuid,
    pub states: BTreeMap<u8, String>,
}

impl Saga {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            states: Default::default(),
        }
    }
    pub fn last_step(&self) -> u8 {
        self.states.last_key_value().map(|(k, _)| *k).unwrap_or(0)
    }
}
