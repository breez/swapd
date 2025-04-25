use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use bitcoin::{
    address::NetworkUnchecked,
    hashes::{sha256, Hash},
    secp256k1::{PublicKey, SecretKey},
    Address, Network, OutPoint, ScriptBuf,
};
use futures::TryStreamExt;
use sqlx::{postgres::PgRow, Executor, PgPool, Postgres, Row};
use tracing::instrument;

use crate::{
    lightning::PaymentResult,
    swap::{
        AddPaymentResultError, GetPaidUtxosError, GetPaymentAttemptsError, GetSwapsError,
        LockSwapError, PaidOutpoint, PaymentAttempt, PaymentAttemptWithResult, Swap, SwapLock,
        SwapPersistenceError, SwapPrivateData, SwapPublicData, SwapState, SwapStatePaidOutpoints,
    },
};

#[derive(Debug)]
pub struct SwapRepository {
    network: Network,
    pool: Arc<PgPool>,
}

impl SwapRepository {
    pub fn new(pool: Arc<PgPool>, network: Network) -> Self {
        Self { pool, network }
    }

    fn map_swap_state(&self, row: &PgRow) -> Result<SwapState, GetSwapsError> {
        let address: &str = row.try_get("address")?;
        let claim_privkey: Vec<u8> = row.try_get("claim_privkey")?;
        let claim_pubkey: Vec<u8> = row.try_get("claim_pubkey")?;
        let claim_script: Vec<u8> = row.try_get("claim_script")?;
        let creation_time: i64 = row.try_get("creation_time")?;
        let lock_time: i32 = row.try_get("lock_time")?;
        let payment_hash: Vec<u8> = row.try_get("payment_hash")?;
        let refund_pubkey: Vec<u8> = row.try_get("refund_pubkey")?;
        let refund_script: Vec<u8> = row.try_get("refund_script")?;

        let creation_time = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(creation_time as u64))
            .ok_or(GetSwapsError::General("invalid timestamp".into()))?;
        let address = address
            .parse::<Address<NetworkUnchecked>>()?
            .require_network(self.network)?;
        let swap = Swap {
            creation_time,
            public: SwapPublicData {
                address: address.clone(),
                claim_pubkey: PublicKey::from_slice(&claim_pubkey)?,
                claim_script: ScriptBuf::from_bytes(claim_script),
                hash: sha256::Hash::from_slice(&payment_hash)?,
                lock_time: lock_time as u16,
                refund_pubkey: PublicKey::from_slice(&refund_pubkey)?,
                refund_script: ScriptBuf::from_bytes(refund_script),
            },
            private: SwapPrivateData {
                claim_privkey: SecretKey::from_slice(&claim_privkey)?,
            },
        };
        let preimage: Option<Vec<u8>> = row.try_get("preimage")?;
        Ok(SwapState {
            swap,
            preimage: match preimage {
                Some(preimage) => Some(
                    preimage
                        .try_into()
                        .map_err(|_| GetSwapsError::InvalidPreimage)?,
                ),
                None => None,
            },
        })
    }
}

#[async_trait::async_trait]
impl crate::swap::SwapRepository for SwapRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_swap(&self, swap: &Swap) -> Result<(), SwapPersistenceError> {
        sqlx::query(
            r#"INSERT INTO swaps (address
               ,                  claim_privkey
               ,                  claim_pubkey
               ,                  claim_script
               ,                  creation_time
               ,                  lock_time
               ,                  payment_hash
               ,                  refund_pubkey
               ,                  refund_script
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        )
        .bind(swap.public.address.to_string())
        .bind(swap.private.claim_privkey.secret_bytes().to_vec())
        .bind(swap.public.claim_pubkey.serialize())
        .bind(swap.public.claim_script.as_bytes())
        .bind(swap.creation_time.duration_since(UNIX_EPOCH)?.as_secs() as i64)
        .bind(swap.public.lock_time as i32)
        .bind(swap.public.hash.as_byte_array().to_vec())
        .bind(swap.public.refund_pubkey.serialize())
        .bind(swap.public.refund_script.as_bytes())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_by_hash(&self, hash: &sha256::Hash) -> Result<SwapState, GetSwapsError> {
        let maybe_row = sqlx::query(&format!(
            r#"SELECT {}
               FROM swaps s
               WHERE s.payment_hash = $1"#,
            swap_state_fields("s")
        ))
        .bind(hash.as_byte_array().to_vec())
        .fetch_optional(&*self.pool)
        .await?;

        let row = match maybe_row {
            Some(row) => row,
            None => return Err(GetSwapsError::NotFound),
        };

        let swap_state = self.map_swap_state(&row)?;
        Ok(swap_state)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_by_address(&self, address: &Address) -> Result<SwapState, GetSwapsError> {
        let maybe_row = sqlx::query(&format!(
            r#"SELECT {}
               FROM swaps s
               WHERE s.address = $1"#,
            swap_state_fields("s"),
        ))
        .bind(address.to_string())
        .fetch_optional(&*self.pool)
        .await?;

        let row = match maybe_row {
            Some(row) => row,
            None => return Err(GetSwapsError::NotFound),
        };

        let swap_state = self.map_swap_state(&row)?;
        Ok(swap_state)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_by_payment_request(
        &self,
        payment_request: &str,
    ) -> Result<SwapState, GetSwapsError> {
        let maybe_row = sqlx::query(&format!(
            r#"SELECT {}
               FROM swaps s
               INNER JOIN payment_attempts p ON s.payment_hash = p.swap_payment_hash
               WHERE p.payment_request = $1
               LIMIT 1"#,
            swap_state_fields("s")
        ))
        .bind(payment_request)
        .fetch_optional(&*self.pool)
        .await?;

        let row = match maybe_row {
            Some(row) => row,
            None => return Err(GetSwapsError::NotFound),
        };

        let swap_state = self.map_swap_state(&row)?;
        Ok(swap_state)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_locks(&self, hash: &sha256::Hash) -> Result<Vec<SwapLock>, LockSwapError> {
        let mut rows = sqlx::query(
            r#"SELECT refund_id
               ,      payment_attempt_label
               FROM swap_locks
               WHERE swap_payment_hash = $1"#,
        )
        .bind(hash.as_byte_array().to_vec())
        .fetch(&*self.pool);

        let mut result = Vec::new();
        while let Some(row) = rows.try_next().await? {
            result.push(SwapLock {
                refund_id: row.try_get("refund_id")?,
                payment_attempt_label: row.try_get("payment_attempt_label")?,
            });
        }

        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_payment_attempts(
        &self,
        hash: &sha256::Hash,
    ) -> Result<Vec<PaymentAttemptWithResult>, GetPaymentAttemptsError> {
        let mut rows = sqlx::query(
            r#"SELECT pa.swap_payment_hash
               ,      pa.label
               ,      pa.creation_time
               ,      pa.amount_msat
               ,      pa.payment_request
               ,      pa.destination
               ,      pa.success
               ,      pa.error
               ,      patx.tx_id
               ,      patx.output_index
               FROM payment_attempts pa
               LEFT JOIN payment_attempt_tx_outputs patx ON pa.id = patx.payment_attempt_id
               WHERE pa.swap_payment_hash = $1
               ORDER BY pa.creation_time"#,
        )
        .bind(hash.as_byte_array().to_vec())
        .fetch(&*self.pool);

        let mut attempts = HashMap::new();
        while let Some(row) = rows.try_next().await? {
            let payment_hash: Vec<u8> = row.try_get("swap_payment_hash")?;
            let label: String = row.try_get("label")?;
            let creation_time: i64 = row.try_get("creation_time")?;
            let amount_msat: i64 = row.try_get("amount_msat")?;
            let payment_request: String = row.try_get("payment_request")?;
            let destination: Vec<u8> = row.try_get("destination")?;
            let success: Option<bool> = row.try_get("success")?;
            let error: Option<String> = row.try_get("error")?;
            let tx_id: Option<String> = row.try_get("tx_id")?;
            let output_index: Option<i64> = row.try_get("output_index")?;

            let payment_hash = sha256::Hash::from_slice(&payment_hash)?;
            let creation_time = SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(creation_time as u64))
                .ok_or(GetPaymentAttemptsError::General("invalid timestamp".into()))?;
            let destination = PublicKey::from_slice(&destination)?;

            let result = match success {
                Some(true) => {
                    let preimage: Option<Vec<u8>> = row.try_get("preimage")?;
                    let preimage = match preimage {
                        Some(preimage) => preimage
                            .try_into()
                            .map_err(|_| GetPaymentAttemptsError::InvalidPreimage)?,
                        None => return Err(GetPaymentAttemptsError::InvalidPreimage),
                    };
                    Some(PaymentResult::Success { preimage })
                }
                Some(false) => Some(PaymentResult::Failure {
                    error: error.unwrap_or_else(|| "unknown error".to_string()),
                }),
                None => None,
            };
            let attempt =
                attempts
                    .entry(label.clone())
                    .or_insert_with(|| PaymentAttemptWithResult {
                        attempt: PaymentAttempt {
                            payment_hash,
                            label: label.clone(),
                            creation_time,
                            amount_msat: amount_msat as u64,
                            payment_request: payment_request.clone(),
                            destination,
                            outputs: Vec::new(),
                        },
                        result,
                    });

            if let (Some(tx_id), Some(output_index)) = (tx_id, output_index) {
                attempt
                    .attempt
                    .outputs
                    .push(OutPoint::new(tx_id.parse()?, output_index as u32));
            }
        }

        Ok(attempts.into_values().collect())
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swaps(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, SwapState>, GetSwapsError> {
        let addresses: Vec<String> = addresses.iter().map(|a| a.to_string()).collect();
        let query = format!(
            r#"SELECT {}
               FROM swaps s
               WHERE s.address = ANY($1)"#,
            swap_state_fields("s")
        );
        let mut rows = sqlx::query(&query).bind(addresses).fetch(&*self.pool);

        let mut result = HashMap::new();
        while let Some(row) = rows.try_next().await? {
            let swap_state = self.map_swap_state(&row)?;
            result.insert(swap_state.swap.public.address.clone(), swap_state);
        }

        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swaps_with_paid_outpoints(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, SwapStatePaidOutpoints>, GetSwapsError> {
        let addresses: Vec<String> = addresses.iter().map(|a| a.to_string()).collect();
        let query = format!(
            r#"SELECT {}
               ,      po.payment_request
               ,      po.tx_id
               ,      po.output_index
               FROM swaps s
               LEFT JOIN (
                   SELECT DISTINCT s_sub.payment_hash
                   ,               pa.payment_request
                   ,               patx.tx_id
                   ,               patx.output_index
                   FROM swaps s_sub
                   INNER JOIN payment_attempts pa ON s_sub.payment_hash = pa.swap_payment_hash
                   INNER JOIN payment_attempt_tx_outputs patx ON pa.id = patx.payment_attempt_id
                   WHERE pa.success = true
               ) po ON s.payment_hash = po.payment_hash
               WHERE s.address = ANY($1)
               ORDER BY s.payment_hash"#,
            swap_state_fields("s")
        );
        let mut rows = sqlx::query(&query).bind(addresses).fetch(&*self.pool);

        let mut result = HashMap::new();
        while let Some(row) = rows.try_next().await? {
            let address: &str = row.try_get("address")?;
            let address = address
                .parse::<Address<NetworkUnchecked>>()?
                .require_network(self.network)?;
            if !result.contains_key(&address) {
                let swap_state = self.map_swap_state(&row)?;
                result.insert(
                    address.clone(),
                    SwapStatePaidOutpoints {
                        paid_outpoints: Vec::new(),
                        swap_state,
                    },
                );
            }

            let tx_id: Option<String> = row.try_get("tx_id")?;
            let output_index: Option<i64> = row.try_get("output_index")?;
            let payment_request: Option<String> = row.try_get("payment_request")?;

            if let (Some(tx_id), Some(output_index), Some(payment_request)) =
                (tx_id, output_index, payment_request)
            {
                let entry = result.get_mut(&address).ok_or(GetSwapsError::General(
                    "missing expected address in map".into(),
                ))?;
                entry.paid_outpoints.push(PaidOutpoint {
                    outpoint: OutPoint::new(tx_id.parse()?, output_index as u32),
                    payment_request,
                });
            }
        }

        Ok(result)
    }

    async fn get_unhandled_payment_attempts(
        &self,
    ) -> Result<Vec<PaymentAttempt>, GetPaymentAttemptsError> {
        let mut rows = sqlx::query(
            r#"SELECT pa.swap_payment_hash
               ,      pa.label
               ,      pa.creation_time
               ,      pa.amount_msat
               ,      pa.payment_request
               ,      pa.destination
               ,      patx.tx_id
               ,      patx.output_index
               FROM payment_attempts pa
               LEFT JOIN payment_attempt_tx_outputs patx ON pa.id = patx.payment_attempt_id
               WHERE pa.success IS NULL
               ORDER BY pa.creation_time"#,
        )
        .fetch(&*self.pool);

        let mut attempts = HashMap::new();
        while let Some(row) = rows.try_next().await? {
            let payment_hash: Vec<u8> = row.try_get("swap_payment_hash")?;
            let label: String = row.try_get("label")?;
            let creation_time: i64 = row.try_get("creation_time")?;
            let amount_msat: i64 = row.try_get("amount_msat")?;
            let payment_request: String = row.try_get("payment_request")?;
            let destination: Vec<u8> = row.try_get("destination")?;
            let tx_id: Option<String> = row.try_get("tx_id")?;
            let output_index: Option<i64> = row.try_get("output_index")?;

            let payment_hash = sha256::Hash::from_slice(&payment_hash)?;
            let creation_time = SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(creation_time as u64))
                .ok_or(GetPaymentAttemptsError::General("invalid timestamp".into()))?;
            let destination = PublicKey::from_slice(&destination)?;

            let attempt = attempts
                .entry(label.clone())
                .or_insert_with(|| PaymentAttempt {
                    payment_hash,
                    label: label.clone(),
                    creation_time,
                    amount_msat: amount_msat as u64,
                    payment_request: payment_request.clone(),
                    destination,
                    outputs: Vec::new(),
                });

            if let (Some(tx_id), Some(output_index)) = (tx_id, output_index) {
                attempt
                    .outputs
                    .push(OutPoint::new(tx_id.parse()?, output_index as u32));
            }
        }

        Ok(attempts.into_values().collect())
    }

    #[instrument(level = "trace", skip(self))]
    async fn lock_add_payment_attempt(
        &self,
        attempt: &PaymentAttempt,
    ) -> Result<(), LockSwapError> {
        let mut tx = self.pool.begin().await?;
        lock_swap_for_update(&attempt.payment_hash, &mut *tx).await?;

        // Payments can only be locked if there is no refund lock and no other payment lock.
        let count: i64 = sqlx::query(
            "SELECT COUNT(*)
             FROM swap_locks 
             WHERE swap_payment_hash = $1",
        )
        .bind(attempt.payment_hash.as_byte_array().to_vec())
        .fetch_one(&mut *tx)
        .await?
        .try_get(0)?;
        if count > 0 {
            return Err(LockSwapError::AlreadyLocked);
        }

        sqlx::query(
            "INSERT INTO swap_locks (swap_payment_hash, payment_attempt_label)
             VALUES($1, $2)",
        )
        .bind(attempt.payment_hash.as_byte_array().to_vec())
        .bind(&attempt.label)
        .execute(&mut *tx)
        .await?;

        let row = sqlx::query(
            r#"
            INSERT INTO payment_attempts (swap_payment_hash
            ,                             label
            ,                             creation_time
            ,                             amount_msat
            ,                             payment_request
            ,                             destination)
            VALUES($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(attempt.payment_hash.as_byte_array().to_vec())
        .bind(&attempt.label)
        .bind(attempt.creation_time.duration_since(UNIX_EPOCH)?.as_secs() as i64)
        .bind(attempt.amount_msat as i64)
        .bind(&attempt.payment_request)
        .bind(attempt.destination.serialize().to_vec())
        .fetch_one(&mut *tx)
        .await?;

        let id: i64 = row.try_get("id")?;
        // Now store the used utxos.
        let tx_ids: Vec<_> = attempt.outputs.iter().map(|o| o.txid.to_string()).collect();
        let output_indices: Vec<_> = attempt.outputs.iter().map(|o| o.vout as i64).collect();
        sqlx::query(
            r#"INSERT INTO payment_attempt_tx_outputs (payment_attempt_id, tx_id, output_index)
               SELECT $1, t.tx_id, t.output_index
               FROM UNNEST($2::text[], $3::bigint[]) 
                   AS t(tx_id, output_index)"#,
        )
        .bind(id)
        .bind(&tx_ids)
        .bind(&output_indices)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn lock_swap_refund(
        &self,
        hash: &sha256::Hash,
        refund_id: &str,
    ) -> Result<(), LockSwapError> {
        let mut tx = self.pool.begin().await?;
        lock_swap_for_update(hash, &mut *tx).await?;

        // Refunds can be locked with another refund lock, but not with a payment lock.
        let count: i64 = sqlx::query(
            "SELECT COUNT(*)
             FROM swap_locks 
             WHERE swap_payment_hash = $1 AND payment_attempt_label IS NOT NULL",
        )
        .bind(hash.as_byte_array().to_vec())
        .fetch_one(&mut *tx)
        .await?
        .try_get(0)?;
        if count > 0 {
            return Err(LockSwapError::AlreadyLocked);
        }

        sqlx::query(
            "INSERT INTO swap_locks (swap_payment_hash, refund_id)
             VALUES($1, $2)",
        )
        .bind(hash.as_byte_array().to_vec())
        .bind(refund_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    async fn unlock_add_payment_result(
        &self,
        hash: &sha256::Hash,
        payment_label: &str,
        result: &PaymentResult,
    ) -> Result<(), LockSwapError> {
        let mut tx = self.pool.begin().await?;
        lock_swap_for_update(hash, &mut *tx).await?;

        sqlx::query(
            "DELETE FROM swap_locks
             WHERE swap_payment_hash = $1 AND payment_attempt_label = $2",
        )
        .bind(hash.as_byte_array().to_vec())
        .bind(payment_label)
        .execute(&mut *tx)
        .await?;

        match result {
            PaymentResult::Success { preimage } => {
                sqlx::query(r#"UPDATE swaps SET preimage = $1 WHERE payment_hash = $2"#)
                    .bind(preimage.to_vec())
                    .bind(hash.as_byte_array().to_vec())
                    .execute(&mut *tx)
                    .await?;

                sqlx::query(r#"UPDATE payment_attempts SET success = true WHERE label = $1"#)
                    .bind(payment_label)
                    .execute(&mut *tx)
                    .await?;
            }
            PaymentResult::Failure { error } => {
                sqlx::query(
                    r#"UPDATE payment_attempts SET success = false, error = $1 WHERE label = $2"#,
                )
                .bind(error)
                .bind(payment_label)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn unlock_swap_refund(
        &self,
        hash: &sha256::Hash,
        refund_id: &str,
    ) -> Result<(), LockSwapError> {
        let mut tx = self.pool.begin().await?;
        lock_swap_for_update(hash, &mut *tx).await?;

        sqlx::query(
            "DELETE FROM swap_locks
             WHERE swap_payment_hash = $1 AND refund_id = $2",
        )
        .bind(hash.as_byte_array().to_vec())
        .bind(refund_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

async fn lock_swap_for_update<'c, E>(hash: &sha256::Hash, executor: E) -> Result<(), LockSwapError>
where
    E: Executor<'c, Database = Postgres>,
{
    // Take a lock on the swap hash, so simultaneous locks will wait.
    let locked_swap_row = sqlx::query("SELECT * FROM swaps WHERE payment_hash = $1 FOR UPDATE")
        .bind(hash.as_byte_array().to_vec())
        .fetch_optional(executor)
        .await?;
    if locked_swap_row.is_none() {
        return Err(LockSwapError::SwapNotFound);
    }

    Ok(())
}

fn swap_state_fields(prefix: &str) -> String {
    format!(
        r#"{0}.address
         , {0}.claim_privkey
         , {0}.claim_pubkey
         , {0}.claim_script
         , {0}.creation_time
         , {0}.lock_time
         , {0}.payment_hash
         , {0}.preimage
         , {0}.refund_pubkey
         , {0}.refund_script
         "#,
        prefix
    )
}

impl From<sqlx::Error> for SwapPersistenceError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::Database(e) => match e.constraint() {
                Some(_) => SwapPersistenceError::AlreadyExists,
                None => SwapPersistenceError::General(Box::new(e)),
            },
            e => SwapPersistenceError::General(Box::new(e)),
        }
    }
}

impl From<SystemTimeError> for SwapPersistenceError {
    fn from(value: SystemTimeError) -> Self {
        SwapPersistenceError::General(Box::new(value))
    }
}

impl From<bitcoin::address::ParseError> for GetSwapsError {
    fn from(value: bitcoin::address::ParseError) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for GetSwapsError {
    fn from(value: sqlx::Error) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::HexToArrayError> for GetSwapsError {
    fn from(value: bitcoin::hashes::hex::HexToArrayError) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::FromSliceError> for GetSwapsError {
    fn from(value: bitcoin::hashes::FromSliceError) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<bitcoin::key::FromSliceError> for GetSwapsError {
    fn from(value: bitcoin::key::FromSliceError) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<bitcoin::secp256k1::Error> for GetSwapsError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for AddPaymentResultError {
    fn from(value: sqlx::Error) -> Self {
        AddPaymentResultError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for GetPaidUtxosError {
    fn from(value: sqlx::Error) -> Self {
        GetPaidUtxosError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for LockSwapError {
    fn from(value: sqlx::Error) -> Self {
        LockSwapError::General(Box::new(value))
    }
}

impl From<SystemTimeError> for LockSwapError {
    fn from(value: SystemTimeError) -> Self {
        LockSwapError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::HexToArrayError> for GetPaidUtxosError {
    fn from(value: bitcoin::hashes::hex::HexToArrayError) -> Self {
        GetPaidUtxosError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for GetPaymentAttemptsError {
    fn from(value: sqlx::Error) -> Self {
        GetPaymentAttemptsError::General(Box::new(value))
    }
}

impl From<bitcoin::secp256k1::Error> for GetPaymentAttemptsError {
    fn from(value: bitcoin::secp256k1::Error) -> Self {
        GetPaymentAttemptsError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::HexToArrayError> for GetPaymentAttemptsError {
    fn from(value: bitcoin::hashes::hex::HexToArrayError) -> Self {
        GetPaymentAttemptsError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::FromSliceError> for GetPaymentAttemptsError {
    fn from(value: bitcoin::hashes::FromSliceError) -> Self {
        GetPaymentAttemptsError::General(Box::new(value))
    }
}
