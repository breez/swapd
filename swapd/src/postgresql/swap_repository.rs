use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use bitcoin::{
    address::NetworkUnchecked,
    hashes::{sha256, Hash},
    Address, Network, OutPoint, PrivateKey, PublicKey, ScriptBuf,
};
use futures::TryStreamExt;
use sqlx::{PgPool, Row};
use tracing::instrument;

use crate::{
    lightning::PaymentResult,
    swap::{
        AddPaymentResultError, AddPreimageError, GetPaidUtxosError, GetSwapError, GetSwapsError,
        PaymentAttempt, Swap, SwapPersistenceError, SwapPrivateData, SwapPublicData, SwapState,
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
}

#[async_trait::async_trait]
impl crate::swap::SwapRepository for SwapRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_swap(&self, swap: &Swap) -> Result<(), SwapPersistenceError> {
        sqlx::query(
            r#"INSERT INTO swaps (creation_time
               ,                  payer_pubkey
               ,                  swapper_pubkey
               ,                  payment_hash
               ,                  script
               ,                  address
               ,                  lock_time
               ,                  swapper_privkey
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(swap.creation_time.duration_since(UNIX_EPOCH)?.as_secs() as i64)
        .bind(swap.public.payer_pubkey.to_bytes())
        .bind(swap.public.swapper_pubkey.to_bytes())
        .bind(swap.public.hash.as_byte_array().to_vec())
        .bind(swap.public.script.to_bytes())
        .bind(swap.public.address.to_string())
        .bind(swap.public.lock_time as i64)
        .bind(swap.private.swapper_privkey.to_bytes())
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

                sqlx::query(r#"UPDATE payment_attempts SET success = 1 WHERE label = $1"#)
                    .bind(label)
                    .execute(&*self.pool)
                    .await?;
            }
            PaymentResult::Failure { error } => {
                sqlx::query(
                    r#"UPDATE payment_attempts SET success = 0, error = $1 WHERE label = $2"#,
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
    async fn get_swap_by_hash(&self, hash: &sha256::Hash) -> Result<SwapState, GetSwapError> {
        let maybe_row = sqlx::query(
            r#"SELECT s.creation_time
               ,      s.payer_pubkey
               ,      s.swapper_pubkey
               ,      s.script
               ,      s.address
               ,      s.lock_time
               ,      s.swapper_privkey
               ,      s.preimage
               FROM swaps s
               WHERE s.payment_hash = $1"#,
        )
        .bind(hash.as_byte_array().to_vec())
        .fetch_optional(&*self.pool)
        .await?;

        let row = match maybe_row {
            Some(row) => row,
            None => return Err(GetSwapError::NotFound),
        };

        let creation_time: i64 = row.try_get("creation_time")?;
        let payer_pubkey: Vec<u8> = row.try_get("payer_pubkey")?;
        let swapper_pubkey: Vec<u8> = row.try_get("swapper_pubkey")?;
        let script: Vec<u8> = row.try_get("script")?;
        let address: &str = row.try_get("address")?;
        let lock_time: i64 = row.try_get("lock_time")?;
        let swapper_privkey: Vec<u8> = row.try_get("swapper_privkey")?;
        let preimage: Option<Vec<u8>> = row.try_get("preimage")?;

        let creation_time = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(creation_time as u64))
            .ok_or(GetSwapError::General("invalid timestamp".into()))?;
        let swap = Swap {
            creation_time,
            public: SwapPublicData {
                address: address
                    .parse::<Address<NetworkUnchecked>>()?
                    .require_network(self.network)?,
                hash: *hash,
                lock_time: lock_time as u32,
                payer_pubkey: PublicKey::from_slice(&payer_pubkey)?,
                swapper_pubkey: PublicKey::from_slice(&swapper_pubkey)?,
                script: ScriptBuf::from_bytes(script),
            },
            private: SwapPrivateData {
                swapper_privkey: PrivateKey::from_slice(&swapper_privkey, self.network)?,
            },
        };

        Ok(SwapState {
            swap,
            preimage: match preimage {
                Some(preimage) => Some(
                    preimage
                        .try_into()
                        .map_err(|_| GetSwapError::InvalidPreimage)?,
                ),
                None => None,
            },
        })
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_by_address(&self, address: &Address) -> Result<SwapState, GetSwapError> {
        let maybe_row = sqlx::query(
            r#"SELECT s.payment_hash
               ,      s.creation_time
               ,      s.payer_pubkey
               ,      s.swapper_pubkey
               ,      s.script
               ,      s.lock_time
               ,      s.swapper_privkey
               ,      s.preimage
               FROM swaps s
               WHERE s.address = $1"#,
        )
        .bind(address.to_string())
        .fetch_optional(&*self.pool)
        .await?;

        let row = match maybe_row {
            Some(row) => row,
            None => return Err(GetSwapError::NotFound),
        };

        let payment_hash: Vec<u8> = row.try_get("payment_hash")?;
        let creation_time: i64 = row.try_get("creation_time")?;
        let payer_pubkey: Vec<u8> = row.try_get("payer_pubkey")?;
        let swapper_pubkey: Vec<u8> = row.try_get("swapper_pubkey")?;
        let script: Vec<u8> = row.try_get("script")?;
        let lock_time: i64 = row.try_get("lock_time")?;
        let swapper_privkey: Vec<u8> = row.try_get("swapper_privkey")?;
        let preimage: Option<Vec<u8>> = row.try_get("preimage")?;

        let creation_time = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(creation_time as u64))
            .ok_or(GetSwapError::General("invalid timestamp".into()))?;
        let swap = Swap {
            creation_time,
            public: SwapPublicData {
                address: address.clone(),
                hash: sha256::Hash::from_slice(&payment_hash)?,
                lock_time: lock_time as u32,
                payer_pubkey: PublicKey::from_slice(&payer_pubkey)?,
                swapper_pubkey: PublicKey::from_slice(&swapper_pubkey)?,
                script: ScriptBuf::from_bytes(script),
            },
            private: SwapPrivateData {
                swapper_privkey: PrivateKey::from_slice(&swapper_privkey, self.network)?,
            },
        };

        Ok(SwapState {
            swap,
            preimage: match preimage {
                Some(preimage) => Some(
                    preimage
                        .try_into()
                        .map_err(|_| GetSwapError::InvalidPreimage)?,
                ),
                None => None,
            },
        })
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_by_payment_request(
        &self,
        payment_request: &str,
    ) -> Result<SwapState, GetSwapError> {
        let maybe_row = sqlx::query(
            r#"SELECT s.payment_hash
               ,      s.creation_time
               ,      s.payer_pubkey
               ,      s.swapper_pubkey
               ,      s.script
               ,      s.address
               ,      s.lock_time
               ,      s.swapper_privkey
               ,      s.preimage
               FROM swaps s
               INNER JOIN payment_attempts p ON s.payment_hash = p.swap_payment_hash
               WHERE p.payment_request = $1
               LIMIT 1"#,
        )
        .bind(payment_request)
        .fetch_optional(&*self.pool)
        .await?;

        let row = match maybe_row {
            Some(row) => row,
            None => return Err(GetSwapError::NotFound),
        };

        let payment_hash: Vec<u8> = row.try_get("payment_hash")?;
        let creation_time: i64 = row.try_get("creation_time")?;
        let payer_pubkey: Vec<u8> = row.try_get("payer_pubkey")?;
        let swapper_pubkey: Vec<u8> = row.try_get("swapper_pubkey")?;
        let script: Vec<u8> = row.try_get("script")?;
        let address: &str = row.try_get("address")?;
        let lock_time: i64 = row.try_get("lock_time")?;
        let swapper_privkey: Vec<u8> = row.try_get("swapper_privkey")?;
        let preimage: Option<Vec<u8>> = row.try_get("preimage")?;

        let creation_time = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(creation_time as u64))
            .ok_or(GetSwapError::General("invalid timestamp".into()))?;
        let swap = Swap {
            creation_time,
            public: SwapPublicData {
                address: address
                    .parse::<Address<NetworkUnchecked>>()?
                    .require_network(self.network)?,
                hash: sha256::Hash::from_slice(&payment_hash)?,
                lock_time: lock_time as u32,
                payer_pubkey: PublicKey::from_slice(&payer_pubkey)?,
                swapper_pubkey: PublicKey::from_slice(&swapper_pubkey)?,
                script: ScriptBuf::from_bytes(script),
            },
            private: SwapPrivateData {
                swapper_privkey: PrivateKey::from_slice(&swapper_privkey, self.network)?,
            },
        };

        Ok(SwapState {
            swap,
            preimage: match preimage {
                Some(preimage) => Some(
                    preimage
                        .try_into()
                        .map_err(|_| GetSwapError::InvalidPreimage)?,
                ),
                None => None,
            },
        })
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_paid_outpoints(
        &self,
        hash: &sha256::Hash,
    ) -> Result<Vec<OutPoint>, GetPaidUtxosError> {
        let mut rows = sqlx::query(
            r#"SELECT DISTINCT patx.tx_id
               ,               patx.output_index
               FROM swaps s
               INNER JOIN payment_attempts pa ON s.payment_hash = pa.swap_payment_hash
               INNER JOIN payment_attempt_tx_outputs patx ON pa.id = patx.payment_attempt_id
               WHERE pa.success = 1"#,
        )
        .bind(hash.as_byte_array().to_vec())
        .fetch(&*self.pool);

        let mut result = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let tx_id: String = row.try_get("tx_id")?;
            let output_index: i64 = row.try_get("output_index")?;
            result.push(OutPoint::new(tx_id.parse()?, output_index as u32));
        }
        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swaps(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, SwapState>, GetSwapsError> {
        let addresses: Vec<String> = addresses.iter().map(|a| a.to_string()).collect();
        let mut rows = sqlx::query(
            r#"SELECT s.creation_time
               ,      s.payment_hash
               ,      s.payer_pubkey
               ,      s.swapper_pubkey
               ,      s.script
               ,      s.address
               ,      s.lock_time
               ,      s.swapper_privkey
               ,      s.preimage
               FROM swaps s
               WHERE s.address = ANY($1)"#,
        )
        .bind(addresses)
        .fetch(&*self.pool);

        let mut result = HashMap::new();
        while let Some(row) = rows.try_next().await? {
            let creation_time: i64 = row.try_get("creation_time")?;
            let payment_hash: Vec<u8> = row.try_get("payment_hash")?;
            let payer_pubkey: Vec<u8> = row.try_get("payer_pubkey")?;
            let swapper_pubkey: Vec<u8> = row.try_get("swapper_pubkey")?;
            let script: Vec<u8> = row.try_get("script")?;
            let address: &str = row.try_get("address")?;
            let lock_time: i64 = row.try_get("lock_time")?;
            let swapper_privkey: Vec<u8> = row.try_get("swapper_privkey")?;
            let preimage: Option<Vec<u8>> = row.try_get("preimage")?;

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
                    hash: sha256::Hash::from_slice(&payment_hash)?,
                    lock_time: lock_time as u32,
                    payer_pubkey: PublicKey::from_slice(&payer_pubkey)?,
                    swapper_pubkey: PublicKey::from_slice(&swapper_pubkey)?,
                    script: ScriptBuf::from_bytes(script),
                },
                private: SwapPrivateData {
                    swapper_privkey: PrivateKey::from_slice(&swapper_privkey, self.network)?,
                },
            };

            result.insert(
                address,
                SwapState {
                    swap,
                    preimage: match preimage {
                        Some(preimage) => Some(
                            preimage
                                .try_into()
                                .map_err(|_| GetSwapsError::InvalidPreimage)?,
                        ),
                        None => None,
                    },
                },
            );
        }

        Ok(result)
    }
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

impl From<sqlx::Error> for AddPreimageError {
    fn from(value: sqlx::Error) -> Self {
        AddPreimageError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::Error> for GetSwapError {
    fn from(value: bitcoin::hashes::hex::Error) -> Self {
        GetSwapError::General(Box::new(value))
    }
}

impl From<bitcoin::address::Error> for GetSwapError {
    fn from(value: bitcoin::address::Error) -> Self {
        GetSwapError::General(Box::new(value))
    }
}

impl From<bitcoin::key::Error> for GetSwapError {
    fn from(value: bitcoin::key::Error) -> Self {
        GetSwapError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for GetSwapError {
    fn from(value: sqlx::Error) -> Self {
        GetSwapError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::Error> for GetSwapError {
    fn from(value: bitcoin::hashes::Error) -> Self {
        GetSwapError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::Error> for GetSwapsError {
    fn from(value: bitcoin::hashes::hex::Error) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::Error> for GetSwapsError {
    fn from(value: bitcoin::hashes::Error) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<bitcoin::address::Error> for GetSwapsError {
    fn from(value: bitcoin::address::Error) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<bitcoin::key::Error> for GetSwapsError {
    fn from(value: bitcoin::key::Error) -> Self {
        GetSwapsError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for GetSwapsError {
    fn from(value: sqlx::Error) -> Self {
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

impl From<bitcoin::hashes::hex::Error> for GetPaidUtxosError {
    fn from(value: bitcoin::hashes::hex::Error) -> Self {
        GetPaidUtxosError::General(Box::new(value))
    }
}
