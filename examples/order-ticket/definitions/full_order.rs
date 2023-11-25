use std::{future::Future, pin::Pin};

use sqlx::{Pool, Postgres, Transaction};
use transaction_state::{
    curry, curry2, curryt,
    definitions::saga_definition::SagaDefinition,
    persisters::persister::{LockScope, StepPersister},
};
use uuid::Uuid;

use crate::{
    models::{
        error::{DefinitionExecutionError, LocalError, TransactionError},
        order::Order,
        ticket::TicketId,
    },
    services::{
        order::{cancel_order, create_order},
        ticket::{create_ticket, send_ticket, TicketConfirmator},
    },
    states::full_order::SagaFullOrderState,
};

pub fn create_full_order<P: StepPersister>(
    pool: Pool<Postgres>,
    persister: P,
    id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaFullOrderState, Option<Order>, TicketId, DefinitionExecutionError, P> {
    let ticket_confirmator = TicketConfirmator {
        pool: pool.clone(),
        success,
    };
    // let cpool = pool.clone();
    SagaDefinition::new(
        LockScope {
            id,
            name: "create_full_order".to_string(),
            executor_id,
        },
        SagaFullOrderState::new,
        id,
        persister,
    )
    .step(
        curryt!(create_order, transaction, pool.clone()),
        // |v| async move {
        //     let mut tx = cpool
        //         .begin()
        //         .await
        //         .map_err(|e| TransactionError(e.to_string()))?;
        //     let r = create_order(&mut tx, v).await;
        //     tx.commit()
        //         .await
        //         .map_err(|e| TransactionError(e.to_string()))?;
        //     r
        // },
        SagaFullOrderState::generate_order_id,
    )
    .step(
        create_ticket,
        SagaFullOrderState::set_order_and_create_ticket,
    )
    .on_error(
        curry!(cancel_order, pool.clone()),
        SagaFullOrderState::cancel_order,
    )
    .step(
        curry2!(TicketConfirmator::confirm_ticket, ticket_confirmator),
        SagaFullOrderState::confirm_ticket,
    )
    .step(curry!(send_ticket, pool), SagaFullOrderState::send_ticket)
}

async fn transaction<
    T,
    C: for<'a> FnOnce(
        &'a mut Transaction<'_, Postgres>,
    ) -> Pin<Box<dyn Future<Output = Result<T, LocalError>> + Send + 'a>>,
>(
    pool: &Pool<Postgres>,
    f: C,
) -> Result<T, LocalError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| LocalError::Transaction(TransactionError(e.to_string())))?;
    let r = f(&mut tx).await?;
    tx.commit()
        .await
        .map_err(|e| LocalError::Transaction(TransactionError(e.to_string())))?;
    Ok(r)
}
