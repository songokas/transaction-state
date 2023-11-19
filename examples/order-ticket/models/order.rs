use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ticket::TicketId;

pub type OrderId = Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Order {
    pub order_id: OrderId,
    pub ticket_id: Option<TicketId>,
}
