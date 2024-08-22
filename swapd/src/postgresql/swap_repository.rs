use std::{str::FromStr, sync::Arc};

use bitcoin::{
    address::NetworkUnchecked,
    hashes::{sha256, Hash},
    Address, Network, OutPoint, PrivateKey, PublicKey, ScriptBuf, Txid,
};
use futures::TryStreamExt;
use sqlx::{PgPool, Row};
use tracing::{instrument, trace};

use crate::{
    chain::Utxo,
    server::{
        self, AddPreimageError, AddressState, GetSwapError, Swap, SwapPersistenceError,
        SwapPrivateData, SwapPublicData, SwapState,
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
impl server::SwapRepository for SwapRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_swap(&self, swap: &Swap) -> Result<(), SwapPersistenceError> {
        sqlx::query(
            r#"INSERT INTO swaps (payer_pubkey
               ,                  swapper_pubkey
               ,                  payment_hash
               ,                  script
               ,                  address
               ,                  lock_time
               ,                  swapper_privkey
               ) VALUES ($1, $2, $3, $4, $5< $6, $7)"#,
        )
        .bind(swap.public.payer_pubkey.to_bytes())
        .bind(swap.public.hash.to_byte_array().to_vec())
        .bind(swap.public.script.to_bytes())
        .bind(swap.public.address.to_string())
        .bind(swap.public.lock_time as i64)
        .bind(swap.private.swapper_privkey.to_bytes())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    async fn add_preimage(&self, swap: &Swap, preimage: &[u8; 32]) -> Result<(), AddPreimageError> {
        let result = sqlx::query("UPDATE swaps SET preimage = $1 WHERE payment_hash = $2")
            .bind(preimage.to_vec())
            .bind(swap.public.hash.to_byte_array().to_vec())
            .execute(&*self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AddPreimageError::DoesNotExist);
        }

        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_swap_state_by_hash(&self, hash: &sha256::Hash) -> Result<SwapState, GetSwapError> {
        let mut rows = sqlx::query(
            r#"SELECT s.payer_pubkey
               ,      s.swapper_pubkey
               ,      s.script
               ,      s.address
               ,      s.lock_time
               ,      s.swapper_privkey
               ,      su.tx_id
               ,      su.output_index
               ,      su.amount
               ,      b.block_hash
               ,      b.height
               FROM swaps s
               LEFT JOIN swap_utxos su ON s.id = su.swap_id
               LEFT JOIN blocks b ON su.block_hash = b.block_hash
               WHERE s.payment_hash = $1"#,
        )
        .bind(hash.to_byte_array().to_vec())
        .fetch(&*self.pool);
        let mut swap: Option<Swap> = None;
        let mut utxos: Vec<Utxo> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            if swap.is_none() {
                let payer_pubkey: Vec<u8> = row.try_get("payer_pubkey")?;
                let swapper_pubkey: Vec<u8> = row.try_get("swapper_pubkey")?;
                let script: Vec<u8> = row.try_get("script")?;
                let address: &str = row.try_get("address")?;
                let lock_time: i64 = row.try_get("lock_time")?;
                let swapper_privkey: Vec<u8> = row.try_get("swapper_privkey")?;

                swap = Some(Swap {
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
                })
            }

            let tx_id = match row.try_get::<Option<&str>, &str>("tx_id")? {
                Some(tx_id) => tx_id,
                None => {
                    trace!("skipping utxo because tx id was not found");
                    continue;
                }
            };
            let output_index: i64 = row.try_get("output_index")?;
            let amount: i64 = row.try_get("amount")?;
            let block_hash = match row.try_get::<Option<&str>, &str>("block_hash")? {
                Some(block_hash) => block_hash,
                None => {
                    trace!(
                        tx_id,
                        output_index,
                        "skipping utxo because block hash was not found"
                    );
                    continue;
                }
            };
            let height: i64 = row.try_get("height")?;
            utxos.push(Utxo {
                block_hash: block_hash.parse()?,
                block_height: height as u32,
                outpoint: OutPoint {
                    txid: Txid::from_str(tx_id)?,
                    vout: output_index as u32,
                },
                amount_sat: amount as u64,
            })
        }

        match swap {
            Some(swap) => Ok(SwapState { swap, utxos }),
            None => Err(GetSwapError::NotFound),
        }
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_state(
        &self,
        addresses: Vec<Address>,
    ) -> Result<Vec<AddressState>, Box<dyn std::error::Error>> {
        todo!()
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
