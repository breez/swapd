use std::{collections::HashMap, sync::Arc};

use bitcoin::{address::NetworkUnchecked, Address, BlockHash, Network, OutPoint};
use futures::TryStreamExt;
use sqlx::{PgPool, Row};

use crate::chain::{self, AddressUtxo, BlockHeader, ChainRepositoryError, SpentUtxo, Utxo};

#[derive(Debug)]
pub struct ChainRepository {
    network: Network,
    pool: Arc<PgPool>,
}

impl ChainRepository {
    pub fn new(pool: Arc<PgPool>, network: Network) -> Self {
        Self { pool, network }
    }
}

// TODO: Ensure when the block is detached, 'things' are no longer returned in queries.
#[async_trait::async_trait]
impl chain::ChainRepository for ChainRepository {
    async fn add_block(&self, block: &BlockHeader) -> Result<(), ChainRepositoryError> {
        sqlx::query(
            r#"INSERT INTO blocks (block_hash, prev_block_hash, height)
               VALUES ($1, $2, $3)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(block.hash.to_string())
        .bind(block.prev.to_string())
        .bind(block.height as i64)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
    async fn add_watch_address(&self, address: &Address) -> Result<(), ChainRepositoryError> {
        sqlx::query(
            r#"INSERT INTO watch_addresses (address)
               VALUES ($1)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(address.to_string())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
    async fn add_watch_addresses(&self, addresses: &[Address]) -> Result<(), ChainRepositoryError> {
        let addresses: Vec<String> = addresses.iter().map(|a| a.to_string()).collect();
        sqlx::query(
            r#"INSERT INTO watch_addresses (address) 
               SELECT * FROM UNNEST($1::text[]) 
               ON CONFLICT DO NOTHING"#,
        )
        .bind(addresses)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
    async fn add_utxo(&self, utxo: &AddressUtxo) -> Result<(), ChainRepositoryError> {
        sqlx::query(
            r#"INSERT INTO address_utxos (
                   address
               ,   tx_id
               ,   output_index
               ,   amount
               ,   block_hash)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(utxo.address.to_string())
        .bind(utxo.utxo.outpoint.txid.to_string())
        .bind(utxo.utxo.outpoint.vout as i64)
        .bind(utxo.utxo.amount_sat as i64)
        .bind(utxo.utxo.block_hash.to_string())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
    async fn add_utxos(&self, utxos: &[AddressUtxo]) -> Result<(), ChainRepositoryError> {
        let addresses: Vec<_> = utxos.iter().map(|u| u.address.to_string()).collect();
        let tx_ids: Vec<_> = utxos
            .iter()
            .map(|u| u.utxo.outpoint.txid.to_string())
            .collect();
        let output_indices: Vec<_> = utxos.iter().map(|u| u.utxo.outpoint.vout as i64).collect();
        let amounts: Vec<_> = utxos.iter().map(|u| u.utxo.amount_sat as i64).collect();
        let block_hashes: Vec<_> = utxos
            .iter()
            .map(|u| u.utxo.block_hash.to_string())
            .collect();
        sqlx::query(
            r#"INSERT INTO address_utxos (
                   address
               ,   tx_id
               ,   output_index
               ,   amount
               ,   block_hash)
               SELECT t.address, t.tx_id, t.output_index, t.amount, t.block_hash 
               FROM UNNEST(
                   $1::text[]
               ,   $2::text[]
               ,   $3::bigint[]
               ,   $4::bigint[]
               ,   $5::text[]
               ) AS t(address, tx_id, output_index, amount, block_hash)
               INNER JOIN watch_addresses w ON w.address = t.address"#,
        )
        .bind(&addresses)
        .bind(&tx_ids)
        .bind(&output_indices)
        .bind(&amounts)
        .bind(&block_hashes)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
    async fn filter_watch_addresses(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Address>, ChainRepositoryError> {
        let addresses: Vec<String> = addresses.iter().map(|a| a.to_string()).collect();
        let mut rows = sqlx::query(
            r#"SELECT address 
               FROM watch_addresses
               WHERE address = ANY($1)"#,
        )
        .bind(addresses)
        .fetch(&*self.pool);

        let mut result: Vec<Address> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let address: String = row.try_get("address")?;
            let address = address
                .parse::<Address<NetworkUnchecked>>()?
                .require_network(self.network)?;
            result.push(address);
        }
        Ok(result)
    }
    async fn get_block_headers(&self) -> Result<Vec<BlockHeader>, ChainRepositoryError> {
        let mut rows = sqlx::query(
            r#"SELECT block_hash
               ,      prev_block_hash
               ,      height
               FROM blocks
               ORDER BY height DESC"#,
        )
        .fetch(&*self.pool);

        let mut result: Vec<BlockHeader> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let block_hash: String = row.try_get("block_hash")?;
            let prev_block_hash: String = row.try_get("prev_block_hash")?;
            let height: i64 = row.try_get("height")?;
            let header = BlockHeader {
                hash: block_hash.parse()?,
                prev: prev_block_hash.parse()?,
                height: height as u64,
            };
            result.push(header);
        }
        Ok(result)
    }

    async fn get_utxos_for_address(
        &self,
        address: &Address,
    ) -> Result<Vec<Utxo>, ChainRepositoryError> {
        let mut rows = sqlx::query(
            r#"SELECT u.tx_id
               ,      u.output_index
               ,      u.amount
               ,      b.block_hash
               ,      b.height
               FROM address_utxos u
               INNER JOIN blocks b ON u.block_hash = b.block_hash
               WHERE u.address = $1
               ORDER BY b.height, u.tx_id, u.output_index"#,
        )
        .bind(address.to_string())
        .fetch(&*self.pool);

        let mut result: Vec<Utxo> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let tx_id: String = row.try_get("tx_id")?;
            let output_index: i64 = row.try_get("output_index")?;
            let amount: i64 = row.try_get("amount")?;
            let block_hash: String = row.try_get("block_hash")?;
            let height: i64 = row.try_get("height")?;
            let utxo = Utxo {
                block_hash: block_hash.parse()?,
                block_height: height as u64,
                outpoint: OutPoint::new(tx_id.parse()?, output_index as u32),
                amount_sat: amount as u64,
            };
            result.push(utxo);
        }
        Ok(result)
    }

    async fn get_utxos_for_addresses(
        &self,
        addresses: &[Address],
    ) -> Result<HashMap<Address, Vec<Utxo>>, ChainRepositoryError> {
        let addresses: Vec<_> = addresses.iter().map(|a| a.to_string()).collect();
        let mut rows = sqlx::query(
            r#"SELECT u.address
               ,      u.tx_id
               ,      u.output_index
               ,      u.amount
               ,      b.block_hash
               ,      b.height
               FROM address_utxos u
               INNER JOIN blocks b ON u.block_hash = b.block_hash
               WHERE u.address = ANY($1)
               ORDER BY u.address, b.height, u.tx_id, u.output_index"#,
        )
        .bind(&addresses)
        .fetch(&*self.pool);

        let mut result: HashMap<Address, Vec<Utxo>> = HashMap::new();
        while let Some(row) = rows.try_next().await? {
            let address: String = row.try_get("address")?;
            let tx_id: String = row.try_get("tx_id")?;
            let output_index: i64 = row.try_get("output_index")?;
            let amount: i64 = row.try_get("amount")?;
            let block_hash: String = row.try_get("block_hash")?;
            let height: i64 = row.try_get("height")?;
            let utxo = Utxo {
                block_hash: block_hash.parse()?,
                block_height: height as u64,
                outpoint: OutPoint::new(tx_id.parse()?, output_index as u32),
                amount_sat: amount as u64,
            };

            let address = address
                .parse::<Address<NetworkUnchecked>>()?
                .require_network(self.network)?;

            let entry = result.entry(address).or_insert(Vec::new());
            entry.push(utxo);
        }

        Ok(result)
    }

    async fn mark_spent(&self, utxos: &[SpentUtxo]) -> Result<(), ChainRepositoryError> {
        let spending_tx_ids: Vec<_> = utxos.iter().map(|u| u.spending_tx.to_string()).collect();
        let spending_block_hashes: Vec<_> =
            utxos.iter().map(|u| u.spending_block.to_string()).collect();
        let utxo_td_ids: Vec<_> = utxos.iter().map(|u| u.outpoint.txid.to_string()).collect();
        let utxo_output_indices: Vec<_> = utxos.iter().map(|u| u.outpoint.vout as i64).collect();
        sqlx::query(
            r#"INSERT INTO spent_utxos (
                           utxo_id
                       ,   spending_tx_id
                       ,   spending_block_hash)
                       SELECT a.utxo_id, t.spending_tx_id, t.spending_block_hash
                       FROM UNNEST(
                           $1::text[]
                       ,   $2::text[]
                       ,   $3::text[]
                       ,   $4::bigint[]
                       ) AS t(
                           spending_tx_id
                       ,   spending_block_hash
                       ,   utxo_tx_id
                       ,   utxo_output_index)
                       INNER JOIN address_utxos a 
                           ON t.utxo_tx_id = a.tx_id 
                               AND t.utxo_output_index = a.output_index"#,
        )
        .bind(&spending_tx_ids)
        .bind(&spending_block_hashes)
        .bind(&utxo_td_ids)
        .bind(&utxo_output_indices)
        .execute(&*self.pool)
        .await?;

        Ok(())
    }
    async fn undo_block(&self, hash: BlockHash) -> Result<(), ChainRepositoryError> {
        sqlx::query("DELETE FROM blocks WHERE block_hash = $1")
            .bind(hash.to_string())
            .execute(&*self.pool)
            .await?;
        Ok(())
    }
}

impl From<bitcoin::address::Error> for ChainRepositoryError {
    fn from(value: bitcoin::address::Error) -> Self {
        ChainRepositoryError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::Error> for ChainRepositoryError {
    fn from(value: bitcoin::hashes::hex::Error) -> Self {
        ChainRepositoryError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for ChainRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        ChainRepositoryError::General(Box::new(value))
    }
}
