use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bitcoin::{
    address::NetworkUnchecked,
    consensus::{Decodable, Encodable},
    hashes::{sha256, Hash},
    Address, Network, Transaction,
};
use sqlx::{PgPool, Row};
use tracing::instrument;

use crate::redeem::{self, Redeem, RedeemRepositoryError};

pub struct RedeemRepository {
    network: Network,
    pool: Arc<PgPool>,
}

impl RedeemRepository {
    pub fn new(pool: Arc<PgPool>, network: Network) -> Self {
        Self { pool, network }
    }
}

#[async_trait::async_trait]
impl redeem::RedeemRepository for RedeemRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_redeem(&self, redeem: &Redeem) -> Result<(), RedeemRepositoryError> {
        let tx_id = redeem.tx.txid().to_string();
        let mut tx: Vec<u8> = Vec::new();
        redeem.tx.consensus_encode(&mut tx)?;
        let mut db_tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO redeems (tx_id, creation_time, tx, destination_address, fee_per_kw, swap_hash)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(&tx_id)
        .bind(redeem.creation_time.duration_since(UNIX_EPOCH)?.as_secs() as i64)
        .bind(tx)
        .bind(redeem.destination_address.to_string())
        .bind(redeem.fee_per_kw as i64)
        .bind(redeem.swap_hash.as_byte_array().to_vec())
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

    #[instrument(level = "trace", skip(self))]
    async fn get_last_redeem(
        &self,
        swap_hash: &sha256::Hash,
    ) -> Result<Option<Redeem>, RedeemRepositoryError> {
        let maybe_row = sqlx::query(
            r#"SELECT r.creation_time
               ,      r.tx
               ,      r.destination_address
               ,      r.fee_per_kw
               FROM redeems r
               WHERE r.swap_hash = $1
               ORDER BY r.creation_time DESC
               LIMIT 1"#,
        )
        .bind(swap_hash.as_byte_array().to_vec())
        .fetch_optional(&*self.pool)
        .await?;

        let row = match maybe_row {
            Some(row) => row,
            None => return Ok(None),
        };

        let creation_time: i64 = row.try_get("creation_time")?;
        let mut tx: &[u8] = row.try_get("tx")?;
        let destination_address: String = row.try_get("destination_address")?;
        let fee_per_kw: i64 = row.try_get("fee_per_kw")?;

        let creation_time = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(creation_time as u64))
            .ok_or(RedeemRepositoryError::InvalidTimestamp)?;
        Ok(Some(Redeem {
            creation_time,
            destination_address: destination_address
                .parse::<Address<NetworkUnchecked>>()?
                .require_network(self.network)?,
            fee_per_kw: fee_per_kw as u32,
            swap_hash: swap_hash.clone(),
            tx: Transaction::consensus_decode(&mut tx)?,
        }))
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

impl From<bitcoin::consensus::encode::Error> for RedeemRepositoryError {
    fn from(value: bitcoin::consensus::encode::Error) -> Self {
        RedeemRepositoryError::General(Box::new(value))
    }
}
