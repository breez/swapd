use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bitcoin::{
    address::NetworkUnchecked,
    consensus::{Decodable, Encodable},
    Address, Network, OutPoint, Transaction,
};
use futures::TryStreamExt;
use sqlx::{PgPool, Row};
use tracing::instrument;

use crate::claim::{self, Claim, ClaimRepositoryError};

pub struct ClaimRepository {
    network: Network,
    pool: Arc<PgPool>,
}

impl ClaimRepository {
    pub fn new(pool: Arc<PgPool>, network: Network) -> Self {
        Self { pool, network }
    }
}

#[async_trait::async_trait]
impl claim::ClaimRepository for ClaimRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_claim(&self, claim: &Claim) -> Result<(), ClaimRepositoryError> {
        let tx_id = claim.tx.compute_txid().to_string();
        let mut tx: Vec<u8> = Vec::new();
        claim.tx.consensus_encode(&mut tx)?;
        let mut db_tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO claims (tx_id, creation_time, tx, destination_address, fee_per_kw, auto_bump)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(&tx_id)
        .bind(claim.creation_time.duration_since(UNIX_EPOCH)?.as_secs() as i64)
        .bind(tx)
        .bind(claim.destination_address.to_string())
        .bind(claim.fee_per_kw as i64)
        .bind(claim.auto_bump)
        .execute(&mut *db_tx)
        .await?;

        let input_tx_ids: Vec<_> = claim
            .tx
            .input
            .iter()
            .map(|i| i.previous_output.txid.to_string())
            .collect();
        let input_tx_outnums: Vec<_> = claim
            .tx
            .input
            .iter()
            .map(|i| i.previous_output.vout as i64)
            .collect();
        sqlx::query(
            r#"INSERT INTO claim_inputs (claim_tx_id, tx_id, output_index)
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

    /// Get all claims where the inputs haven't been spent yet, sorted by fee
    /// rate desc, then creation time desc.
    #[instrument(level = "trace", skip(self))]
    async fn get_claims(&self, outpoints: &[OutPoint]) -> Result<Vec<Claim>, ClaimRepositoryError> {
        // NOTE: This query violates the separation principle of separating
        // chain and claim logic.
        let mut rows = sqlx::query(
            r#"SELECT r.creation_time
               ,      r.tx
               ,      r.destination_address
               ,      r.fee_per_kw
               ,      r.auto_bump
               FROM claims r
               WHERE tx_id IN (
                   SELECT ri.claim_tx_id
                   FROM claim_inputs ri
                   WHERE ri.tx_id NOT IN (
                       SELECT ti.tx_id
                       FROM tx_inputs ti
                       INNER JOIN tx_blocks tb ON ti.spending_tx_id = tb.tx_id
                       INNER JOIN blocks b ON tb.block_hash = b.block_hash
                   )
               )
               ORDER BY r.fee_per_kw DESC, r.creation_time DESC"#,
        )
        .fetch(&*self.pool);

        let mut result = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let creation_time: i64 = row.try_get("creation_time")?;
            let mut tx: &[u8] = row.try_get("tx")?;
            let destination_address: String = row.try_get("destination_address")?;
            let fee_per_kw: i64 = row.try_get("fee_per_kw")?;
            let auto_bump: bool = row.try_get("auto_bump")?;

            let creation_time = SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(creation_time as u64))
                .ok_or(ClaimRepositoryError::InvalidTimestamp)?;
            result.push(Claim {
                creation_time,
                destination_address: destination_address
                    .parse::<Address<NetworkUnchecked>>()?
                    .require_network(self.network)?,
                fee_per_kw: fee_per_kw as u32,
                tx: Transaction::consensus_decode(&mut tx)?,
                auto_bump,
            });
        }

        Ok(result)
    }
}

impl From<bitcoin::address::ParseError> for ClaimRepositoryError {
    fn from(value: bitcoin::address::ParseError) -> Self {
        ClaimRepositoryError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::HexToArrayError> for ClaimRepositoryError {
    fn from(value: bitcoin::hashes::hex::HexToArrayError) -> Self {
        ClaimRepositoryError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for ClaimRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        ClaimRepositoryError::General(Box::new(value))
    }
}

impl From<std::io::Error> for ClaimRepositoryError {
    fn from(value: std::io::Error) -> Self {
        ClaimRepositoryError::General(Box::new(value))
    }
}

impl From<std::time::SystemTimeError> for ClaimRepositoryError {
    fn from(value: std::time::SystemTimeError) -> Self {
        ClaimRepositoryError::General(Box::new(value))
    }
}

impl From<bitcoin::io::Error> for ClaimRepositoryError {
    fn from(value: bitcoin::io::Error) -> Self {
        ClaimRepositoryError::General(Box::new(value))
    }
}

impl From<bitcoin::consensus::encode::Error> for ClaimRepositoryError {
    fn from(value: bitcoin::consensus::encode::Error) -> Self {
        ClaimRepositoryError::General(Box::new(value))
    }
}
