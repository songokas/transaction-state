## Howto

Create definitions and run it

```rust
pub fn create_definition_for_existing_order<P: StepPersister>(
    pool: Pool<Postgres>,
    persister: P,
    order_id: Uuid,
    success: bool,
    executor_id: Uuid,
) -> SagaDefinition<SagaOrderState, Order, TicketId, DefinitionExecutionError, P> {
    let ticket_confirmator = TicketConfirmator {
        pool: pool.clone(),
        success,
    };
    SagaDefinition::new(
        LockScope {
            id: order_id,
            name: "create_from_existing_order".to_string(),
            executor_id,
        },
        SagaOrderState::new,
        (),
        persister,
    )
    .step(create_ticket, SagaOrderState::create_ticket)
    .on_error(
        curry!(cancel_order, pool.clone()),
        SagaOrderState::cancel_order,
    )
    .step(
        curry2!(TicketConfirmator::confirm_ticket, ticket_confirmator),
        SagaOrderState::confirm_ticket,
    )
    .step(curry!(send_ticket, pool), SagaOrderState::send_ticket)
}
``

Resume definitions in case of a failure in a separate thread/instance

```rust
pub async fn run_definition<P: StepPersister + Clone + Send + 'static>(
    pool: Pool<Postgres>,
    persister: P,
    name: String,
    id: Uuid,
    executor_id: Uuid,
) -> Result<(), DefinitionExecutionError> {
    match name.as_str() {
        "create_from_existing_order" => {
            let c = create_definition_for_existing_order(pool, persister, id, true, executor_id);
            let r: Result<EmailId, DefinitionExecutionError> = c.continue_from_last_step().await;
            log::info!("Retried email {name} {id} {r:?}");
            Ok(())
        }
        "create_full_order" => {
            let r: Result<EmailId, DefinitionExecutionError> = {
                let a = create_full_order(pool, persister, id, true, executor_id);
                a.continue_from_last_step().await
            };
            log::info!("Retried email {name} {id} {r:?}");
            Ok(())
        }
        _ => panic!("unknown definition {name}"),
    }
}
```

Check [examples/order-ticket](examples/order-ticket) for more info