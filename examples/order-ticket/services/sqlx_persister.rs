use std::time::Duration;

use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Transaction};
use transaction_state::{
    definitions::saga::Saga,
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
                PersistError::Execution(e.to_string(), "store transaction".to_string())
            })?;
        let result = lock(&mut tx, scope, lock_type, self.lock_timeout).await;
        tx.commit()
            .await
            .map_err(|e| PersistError::Execution(e.to_string(), "store comit".to_string()))?;
        result
    }

    async fn retrieve(&self, id: Uuid) -> Result<Saga, PersistError> {
        let rows: Vec<(i16, String)> =
            sqlx::query_as("SELECT step, state FROM saga_step WHERE id = $1")
                .bind(id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PersistError::Execution(e.to_string(), "retrieve".to_string()))?;
        let states = rows.into_iter().map(|row| (row.0 as u8, row.1)).collect();
        Ok(Saga { id, states })
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
        sqlx::query_as::<_, (Uuid, String, Uuid)>(
            "SELECT id, name, executor_id FROM saga_lock WHERE (dtc > $1 OR lock = $2) AND lock != $3 ORDER BY dtc DESC LIMIT 1"
        )
            .bind(for_duration)
            .bind(SqlxLockType::Failed)
            .bind(SqlxLockType::Finished)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PersistError::Execution(e.to_string(), "retrieve failed".to_string()))
    }
}

// TODO trait
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
            // TODO delete or not
            // sqlx::query("DELETE FROM saga_lock WHERE id = ?")
            //     .bind(scope.id)
            //     .execute(&self.pool)
            //     .await?;
            // sqlx::query("DELETE FROM saga_step WHERE id = ?")
            //     .bind(scope.id)
            //     .execute(&self.pool)
            //     .await?;
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
            VALUES ($1, $2, $3)",
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

impl From<SqlxLockType> for LockType {
    fn from(value: SqlxLockType) -> Self {
        match value {
            SqlxLockType::Executing => LockType::Executing,
            SqlxLockType::Failed => LockType::Failed,
            SqlxLockType::Finished => LockType::Finished,
            SqlxLockType::Initial => LockType::Initial,
            SqlxLockType::Retry => LockType::Retry,
        }
    }
}
