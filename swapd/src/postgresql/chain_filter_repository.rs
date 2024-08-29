use std::sync::Arc;

use bitcoin::Address;
use futures::TryStreamExt;
use sqlx::PgPool;
use tracing::instrument;

use crate::chain_filter;

#[derive(Debug)]
pub struct ChainFilterRepository {
    pool: Arc<PgPool>,
}

impl ChainFilterRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl chain_filter::ChainFilterRepository for ChainFilterRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_filter_addresses(
        &self,
        addresses: &[Address],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addresses: Vec<String> = addresses.iter().map(|a| a.to_string()).collect();
        sqlx::query(
            r#"INSERT INTO filter_addresses (address) 
               SELECT * FROM UNNEST($1::text[]) 
               ON CONFLICT DO NOTHING"#,
        )
        .bind(addresses)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    async fn has_filtered_address(
        &self,
        addresses: &[Address],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let addresses: Vec<String> = addresses.iter().map(|a| a.to_string()).collect();
        let mut rows = sqlx::query(
            r#"SELECT address
               FROM filter_addresses
               WHERE address = ANY($1)"#,
        )
        .bind(addresses)
        .fetch(&*self.pool);

        let mut result = false;
        while (rows.try_next().await?).is_some() {
            result = true;
        }

        return Ok(result);
    }
}
