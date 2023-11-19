use std::sync::{Mutex, Arc};

use crate::models::{order::{Order, OrderId}, ticket::TicketId};

#[derive(Clone)]
pub struct SagaFullOrderState {
    order: Arc<Mutex<Option<Order>>>,
}

impl SagaFullOrderState {
    pub fn new(_order: Option<Order>) -> Self {
        Self {
            order: Arc::new(Mutex::new(None)),
        }
    }

    pub fn generate_order_id(&self, order_id: OrderId) -> OrderId {
        order_id
    }

    pub fn set_order_and_create_ticket(&self, order: Order) -> Order {
        *self.order.lock().unwrap() = Some(order.clone());
        order
    }

    pub fn confirm_ticket(&self, ticket_id: TicketId) -> (Order, TicketId) {
        (self.order.lock().unwrap().clone().unwrap(), ticket_id)
    }

    pub fn send_ticket(&self, order: Order) -> Order {
        order
    }
}
