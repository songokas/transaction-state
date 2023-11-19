use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type EmailId = Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TickeEmail {
    email_id: EmailId,
}
