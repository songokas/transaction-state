use std::{future::Future, pin::Pin};

use sqlx::{Pool, Postgres, Transaction};

use crate::models::error::{LocalError, TransactionError};

pub async fn execute_transaction<
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
