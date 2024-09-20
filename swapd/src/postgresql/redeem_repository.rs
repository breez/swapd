use std::{sync::Arc, time::UNIX_EPOCH};

use bitcoin::consensus::Encodable;
use sqlx::PgPool;

use crate::redeem::{self, Redeem, RedeemRepositoryError};

pub struct RedeemRepository {
    pool: Arc<PgPool>,
}

impl RedeemRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl redeem::RedeemRepository for RedeemRepository {
    async fn add_redeem(&self, redeem: &Redeem) -> Result<(), RedeemRepositoryError> {
        let tx_id = redeem.tx.txid().to_string();
        let mut tx: Vec<u8> = Vec::new();
        redeem.tx.consensus_encode(&mut tx)?;
        let mut db_tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO redeems (tx_id, creation_time, tx, destination_address, fee_per_kw)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(&tx_id)
        .bind(redeem.creation_time.duration_since(UNIX_EPOCH)?.as_secs() as i64)
        .bind(tx)
        .bind(redeem.destination_address.to_string())
        .bind(redeem.fee_per_kw as i64)
        .execute(&mut *db_tx)
        .await?;

        let input_tx_ids: Vec<_> = redeem
            .tx
            .input
            .iter()
            .map(|i| i.previous_output.txid.to_string())
            .collect();
        let input_tx_outnums: Vec<_> = redeem
            .tx
            .input
            .iter()
            .map(|i| i.previous_output.vout as i64)
            .collect();
        sqlx::query(
            r#"INSERT INTO redeem_inputs (redeem_tx_id, tx_id, output_index)
               SELECT $1, t.tx_id, t.output_index
               FROM UNNEST($2::text[], $3::bigint[]) 
                   AS t(tx_id, output_index)"#,
        )
        .bind(&tx_id)
        .bind(input_tx_ids)
        .bind(input_tx_outnums)
        .execute(&mut *db_tx)
        .await?;

        Ok(())
    }
}

impl From<bitcoin::address::Error> for RedeemRepositoryError {
    fn from(value: bitcoin::address::Error) -> Self {
        RedeemRepositoryError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::Error> for RedeemRepositoryError {
    fn from(value: bitcoin::hashes::hex::Error) -> Self {
        RedeemRepositoryError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for RedeemRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        RedeemRepositoryError::General(Box::new(value))
    }
}

impl From<std::io::Error> for RedeemRepositoryError {
    fn from(value: std::io::Error) -> Self {
        RedeemRepositoryError::General(Box::new(value))
    }
}

impl From<std::time::SystemTimeError> for RedeemRepositoryError {
    fn from(value: std::time::SystemTimeError) -> Self {
        RedeemRepositoryError::General(Box::new(value))
    }
}
