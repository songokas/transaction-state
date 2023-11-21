use crate::models::{order::Order, ticket::TicketId};

#[derive(Debug, Clone)]
pub struct SagaOrderState {
    order: Order,
}

impl SagaOrderState {
    pub fn new(order: Order, _: &()) -> SagaOrderState {
        Self { order }
    }

    pub fn create_ticket(&self, _: ()) -> Order {
        self.order.clone()
    }

    pub fn confirm_ticket(&self, ticket_id: TicketId) -> (Order, TicketId) {
        (self.order.clone(), ticket_id)
    }

    pub fn send_ticket(&self, order: Order) -> Order {
        order
    }
}
