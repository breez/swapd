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
use sqlx::{postgres::PgRow, PgPool, Row};
use tracing::instrument;

use crate::{
    lightning::PaymentResult,
    swap::{
        AddPaymentResultError, GetPaidUtxosError, GetSwapsError, PaidOutpoint, PaymentAttempt,
        Swap, SwapPersistenceError, SwapPrivateData, SwapPublicData, SwapState,
        SwapStatePaidOutpoints,
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
        let lock_height: i64 = row.try_get("lock_height")?;
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
                lock_height: lock_height as u32,
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
               ,                  lock_height
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
        .bind(swap.public.lock_height as i64)
        .bind(swap.public.hash.as_byte_array().to_vec())
        .bind(swap.public.refund_pubkey.serialize())
        .bind(swap.public.refund_script.as_bytes())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    async fn add_payment_attempt(
        &self,
        attempt: &PaymentAttempt,
    ) -> Result<(), SwapPersistenceError> {
        let mut tx = self.pool.begin().await?;
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
        let tx_ids: Vec<_> = attempt
            .utxos
            .iter()
            .map(|u| u.outpoint.txid.to_string())
            .collect();
        let output_indices: Vec<_> = attempt
            .utxos
            .iter()
            .map(|u| u.outpoint.vout as i64)
            .collect();
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

    #[instrument(level = "trace", skip(self))]
    async fn add_payment_result(
        &self,
        hash: &sha256::Hash,
        label: &str,
        result: &PaymentResult,
    ) -> Result<(), AddPaymentResultError> {
        match result {
            PaymentResult::Success { preimage } => {
                sqlx::query(r#"UPDATE swaps SET preimage = $1 WHERE payment_hash = $2"#)
                    .bind(preimage.to_vec())
                    .bind(hash.as_byte_array().to_vec())
                    .execute(&*self.pool)
                    .await?;

                sqlx::query(r#"UPDATE payment_attempts SET success = true WHERE label = $1"#)
                    .bind(label)
                    .execute(&*self.pool)
                    .await?;
            }
            PaymentResult::Failure { error } => {
                sqlx::query(
                    r#"UPDATE payment_attempts SET success = false, error = $1 WHERE label = $2"#,
                )
                .bind(error)
                .bind(label)
                .execute(&*self.pool)
                .await?;
            }
        }

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
}

fn swap_state_fields(prefix: &str) -> String {
    format!(
        r#"{0}.address
         , {0}.claim_privkey
         , {0}.claim_pubkey
         , {0}.claim_script
         , {0}.creation_time
         , {0}.lock_height
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

impl From<bitcoin::hashes::hex::HexToArrayError> for GetPaidUtxosError {
    fn from(value: bitcoin::hashes::hex::HexToArrayError) -> Self {
        GetPaidUtxosError::General(Box::new(value))
    }
}
