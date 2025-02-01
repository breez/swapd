use std::sync::Arc;

use sqlx::{PgPool, Row};
use tracing::instrument;

use crate::lnd::{self, RepositoryError};

#[derive(Debug)]
pub struct LndRepository {
    pool: Arc<PgPool>,
}

impl LndRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl lnd::Repository for LndRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_label(&self, label: String, payment_index: u64) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO lnd_payments (label, payment_index)
               VALUES($1, $2)"#,
        )
        .bind(label)
        .bind(payment_index as i64)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_label(&self, payment_index: u64) -> Result<Option<String>, RepositoryError> {
        let row = sqlx::query(
            r#"SELECT label 
               FROM lnd_payments
               WHERE payment_index = $1"#,
        )
        .bind(payment_index as i64)
        .fetch_optional(&*self.pool)
        .await?;

        let row = match row {
            Some(row) => row,
            None => return Ok(None),
        };

        let label: String = row.try_get("label")?;
        Ok(Some(label))
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_payment_index(&self, label: &str) -> Result<Option<u64>, RepositoryError> {
        let row = sqlx::query(
            r#"SELECT payment_index 
               FROM lnd_payments
               WHERE label = $1"#,
        )
        .bind(label)
        .fetch_optional(&*self.pool)
        .await?;

        let row = match row {
            Some(row) => row,
            None => return Ok(None),
        };

        let payment_index: i64 = row.try_get("payment_index")?;
        Ok(Some(payment_index as u64))
    }
}

impl From<sqlx::Error> for RepositoryError {
    fn from(value: sqlx::Error) -> Self {
        RepositoryError::General(Box::new(value))
    }
}
