use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Transaction};
use transaction_state::{
    definitions::saga_state::SagaState,
    persisters::persister::{LockScope, LockType, PersistError, StepPersister},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SqlxPersister {
    pool: Pool<Postgres>,
    lock_timeout: Duration,
}

impl SqlxPersister {
    pub fn new(pool: Pool<Postgres>, lock_timeout: Duration) -> Self {
        Self { pool, lock_timeout }
    }
}

#[async_trait::async_trait]
impl StepPersister for SqlxPersister {
    async fn lock(&self, scope: LockScope, lock_type: LockType) -> Result<(), PersistError> {
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                PersistError::Execution(e.to_string(), "lock transaction".to_string())
            })?;
        let result = lock(&mut tx, scope, lock_type, self.lock_timeout).await;
        tx.commit()
            .await
            .map_err(|e| PersistError::Execution(e.to_string(), "lock".to_string()))?;
        result
    }

    async fn retrieve(&self, id: Uuid) -> Result<SagaState, PersistError> {
        let rows: Vec<(i16, String)> =
            sqlx::query_as("SELECT step, state FROM saga_step WHERE id = $1")
                .bind(id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PersistError::Execution(e.to_string(), "retrieve".to_string()))?;
        let states = rows.into_iter().map(|row| (row.0 as u8, row.1)).collect();
        Ok(SagaState {
            id,
            states,
            cancelled: false,
        })
    }

    async fn store(&self, id: Uuid, step: u8, state: String) -> Result<(), PersistError> {
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                PersistError::Execution(e.to_string(), "store transaction".to_string())
            })?;
        let result = store(&mut tx, id, step, state).await;
        tx.commit()
            .await
            .map_err(|e| PersistError::Execution(e.to_string(), "store comit".to_string()))?;
        result
    }

    async fn get_next_failed(
        &self,
        for_duration: Duration,
    ) -> Result<Option<(Uuid, String, Uuid)>, PersistError> {
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                PersistError::Execution(e.to_string(), "store transaction".to_string())
            })?;
        let result = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM saga_lock WHERE (dtc < $1 OR lock = $2) AND lock != $3 ORDER BY dtc DESC LIMIT 1"
        )
            .bind(Utc::now().naive_utc() - for_duration)
            .bind(SqlxLockType::Failed)
            .bind(SqlxLockType::Finished)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistError::Execution(e.to_string(), "retrieve failed".to_string()))?;

        if let Some((id, name)) = result {
            let executor_id = Uuid::new_v4();
            let scope = LockScope {
                id,
                executor_id,
                name,
            };
            lock(&mut tx, scope.clone(), LockType::Retry, for_duration).await?;
            tx.commit()
                .await
                .map_err(|e| PersistError::Execution(e.to_string(), "get commit".to_string()))?;

            Ok(Some((scope.id, scope.name, executor_id)))
        } else {
            Ok(None)
        }
    }
}

pub async fn save_initial_state<S: Serialize + Send + Sync>(
    tx: &mut Transaction<'_, Postgres>,
    scope: LockScope,
    initial_state: &S,
    lock_timeout: Duration,
) -> Result<(), PersistError> {
    let state = serde_json::to_string(initial_state)?;
    lock(tx, scope.clone(), LockType::Initial, lock_timeout).await?;
    store(tx, scope.id, 0, state).await?;
    Ok(())
}

async fn lock(
    tx: &mut Transaction<'_, Postgres>,
    scope: LockScope,
    lock_type: LockType,
    lock_timeout: Duration,
) -> Result<(), PersistError> {
    let row: Option<(Uuid, SqlxLockType, NaiveDateTime)> = sqlx::query_as(
        "SELECT executor_id, lock, dtc FROM saga_lock WHERE id = $1 ORDER BY dtc DESC LIMIT 1",
    )
    .bind(scope.id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| PersistError::Execution(e.to_string(), "retrieve lock".to_string()))?;

    let insert = if let Some(context) = row {
        scope.executor_id == context.0
            || matches!(context.1, SqlxLockType::Failed)
            || Utc::now().naive_utc() > context.2 + lock_timeout
    } else {
        true
    };

    if insert {
        if matches!(lock_type, LockType::Finished) {
            sqlx::query("DELETE FROM saga_lock WHERE id = $1")
                .bind(scope.id)
                .execute(&mut **tx)
                .await
                .map_err(|e| {
                    PersistError::Execution(e.to_string(), "finished saga lock".to_string())
                })?;
            sqlx::query("DELETE FROM saga_step WHERE id = $1")
                .bind(scope.id)
                .execute(&mut **tx)
                .await
                .map_err(|e| {
                    PersistError::Execution(e.to_string(), "finished saga step".to_string())
                })?;
        } else {
            sqlx::query(
                "INSERT INTO saga_lock (id, executor_id, name, lock, dtc)
                    VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(scope.id)
            .bind(scope.executor_id)
            .bind(scope.name)
            .bind(SqlxLockType::from(lock_type))
            .bind(Utc::now().naive_utc())
            .execute(&mut **tx)
            .await
            .map_err(|e| PersistError::Execution(e.to_string(), "insert lock".to_string()))?;
        }
        Ok(())
    } else {
        Err(PersistError::Locked)
    }
}

async fn store(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    step: u8,
    state: String,
) -> Result<(), PersistError> {
    sqlx::query(
        "INSERT INTO saga_step (id, step, state)
            VALUES ($1, $2, $3)
            ",
    )
    .bind(id)
    .bind(step as i16)
    .bind(state)
    .execute(&mut **tx)
    .await
    .map(|_| ())
    .map_err(|e| PersistError::Execution(e.to_string(), "store step".to_string()))
}

#[derive(Serialize, Deserialize, Debug, sqlx::Type)]
#[sqlx(type_name = "lock_type")]
enum SqlxLockType {
    Executing,
    Failed,
    Finished,
    Initial,
    Retry,
}

impl From<LockType> for SqlxLockType {
    fn from(value: LockType) -> Self {
        match value {
            LockType::Executing => SqlxLockType::Executing,
            LockType::Failed => SqlxLockType::Failed,
            LockType::Finished => SqlxLockType::Finished,
            LockType::Initial => SqlxLockType::Initial,
            LockType::Retry => SqlxLockType::Retry,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use sqlx::postgres::PgPoolOptions;

    use super::*;

    #[tokio::test]
    async fn test_same_executor_can_always_lock() {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy("postgres://postgres:123456@localhost/order_ticket_test")
            .unwrap();
        let persister = SqlxPersister::new(pool, Duration::from_millis(10));
        let scope = LockScope {
            id: Uuid::new_v4(),
            executor_id: Uuid::new_v4(),
            name: "test1".to_string(),
        };
        persister
            .lock(scope.clone(), LockType::Initial)
            .await
            .unwrap();
        persister
            .lock(scope.clone(), LockType::Failed)
            .await
            .unwrap();
        persister
            .lock(scope.clone(), LockType::Retry)
            .await
            .unwrap();
        persister
            .lock(scope.clone(), LockType::Executing)
            .await
            .unwrap();
        persister.lock(scope, LockType::Finished).await.unwrap();
    }

    #[tokio::test]
    async fn test_different_executor_can_lock_conditionally() {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy("postgres://postgres:123456@localhost/order_ticket_test")
            .unwrap();
        let persister = SqlxPersister::new(pool, Duration::from_millis(100));
        let scope1 = LockScope {
            id: Uuid::new_v4(),
            executor_id: Uuid::new_v4(),
            name: "test1".to_string(),
        };
        let scope2 = LockScope {
            id: scope1.id,
            executor_id: Uuid::new_v4(),
            name: "test1".to_string(),
        };
        persister
            .lock(scope1.clone(), LockType::Initial)
            .await
            .unwrap();

        let result = persister.lock(scope2.clone(), LockType::Executing).await;
        assert!(matches!(result, Err(PersistError::Locked)), "{result:?}");

        sleep(Duration::from_millis(113));

        let result = persister.lock(scope2.clone(), LockType::Failed).await;
        assert!(result.is_ok(), "{result:?}");

        let result = persister.lock(scope1.clone(), LockType::Executing).await;
        assert!(result.is_ok(), "{result:?}");
    }
}
