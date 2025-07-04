use std::sync::Arc;

use bitcoin::{address::NetworkUnchecked, Address, Amount, BlockHash, Network, OutPoint, TxOut};
use futures::TryStreamExt;
use sqlx::{postgres::PgRow, PgConnection, PgPool, Row};
use tracing::instrument;

use crate::chain::{
    self, AddressUtxo, BlockHeader, ChainRepositoryError, SpentTxo, Txo, TxoSpend, TxoWithSpend,
};

#[derive(Debug)]
pub struct ChainRepository {
    network: Network,
    pool: Arc<PgPool>,
}

impl ChainRepository {
    pub fn new(pool: Arc<PgPool>, network: Network) -> Self {
        Self { pool, network }
    }

    #[instrument(level = "trace", skip(self))]
    async fn add_utxos(
        &self,
        tx: &mut PgConnection,
        tx_outputs: &[AddressUtxo],
    ) -> Result<(), ChainRepositoryError> {
        let tx_ids: Vec<_> = tx_outputs
            .iter()
            .map(|u| u.utxo.outpoint.txid.to_string())
            .collect();
        let output_indices: Vec<_> = tx_outputs
            .iter()
            .map(|u| u.utxo.outpoint.vout as i64)
            .collect();
        let addresses: Vec<_> = tx_outputs.iter().map(|u| u.address.to_string()).collect();
        let amounts: Vec<_> = tx_outputs
            .iter()
            .map(|u| u.utxo.tx_out.value.to_sat() as i64)
            .collect();
        sqlx::query(
            r#"INSERT INTO tx_outputs (
                   tx_id
               ,   output_index
               ,   address
               ,   amount)
               SELECT t.tx_id, t.output_index, t.address, t.amount
               FROM UNNEST(
                   $1::text[]
               ,   $2::bigint[]
               ,   $3::text[]
               ,   $4::bigint[]
               ) AS t(tx_id, output_index, address, amount)
               INNER JOIN watch_addresses w ON w.address = t.address
               ON CONFLICT DO NOTHING"#,
        )
        .bind(&tx_ids)
        .bind(&output_indices)
        .bind(&addresses)
        .bind(&amounts)
        .execute(tx)
        .await?;

        Ok(())
    }

    async fn map_txo(&self, address: &Address, row: &PgRow) -> Result<Txo, ChainRepositoryError> {
        let tx_id: String = row.try_get("tx_id")?;
        let output_index: i64 = row.try_get("output_index")?;
        let amount: i64 = row.try_get("amount")?;
        let block_hash: String = row.try_get("block_hash")?;
        let height: i64 = row.try_get("height")?;
        Ok(Txo {
            block_hash: block_hash.parse()?,
            block_height: height as u64,
            outpoint: OutPoint::new(tx_id.parse()?, output_index as u32),
            tx_out: TxOut {
                value: Amount::from_sat(amount as u64),
                script_pubkey: address.script_pubkey(),
            },
        })
    }

    #[instrument(level = "trace", skip(self))]
    async fn mark_spent(
        &self,
        tx: &mut PgConnection,
        txos: &[SpentTxo],
    ) -> Result<Vec<SpentTxo>, ChainRepositoryError> {
        let tx_ids: Vec<_> = txos.iter().map(|u| u.outpoint.txid.to_string()).collect();
        let tx_output_indices: Vec<_> = txos.iter().map(|u| u.outpoint.vout as i64).collect();
        let spending_tx_ids: Vec<_> = txos.iter().map(|u| u.spending_tx.to_string()).collect();
        let spending_tx_input_indices: Vec<_> =
            txos.iter().map(|u| u.spending_input_index as i64).collect();

        let mut rows = sqlx::query(
            r#"INSERT INTO tx_inputs (
                   tx_id
               ,   output_index
               ,   spending_tx_id
               ,   spending_input_index)
               SELECT i.tx_id
               ,      i.output_index
               ,      i.spending_tx_id
               ,      i.spending_input_index
               FROM UNNEST(
                   $1::text[]
               ,   $2::bigint[]
               ,   $3::text[]
               ,   $4::bigint[]
               ) AS i (
                   tx_id
               ,   output_index
               ,   spending_tx_id
               ,   spending_input_index)
               INNER JOIN tx_outputs o 
                   ON i.tx_id = o.tx_id AND i.output_index = o.output_index
               ON CONFLICT DO NOTHING
               RETURNING tx_id, output_index, spending_tx_id, spending_input_index"#,
        )
        .bind(&tx_ids)
        .bind(&tx_output_indices)
        .bind(&spending_tx_ids)
        .bind(&spending_tx_input_indices)
        .fetch(tx);

        let mut result: Vec<SpentTxo> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let tx_id: String = row.try_get("tx_id")?;
            let output_index: i64 = row.try_get("output_index")?;
            let spending_tx_id: String = row.try_get("spending_tx_id")?;
            let spending_input_index: i64 = row.try_get("spending_input_index")?;
            let outpoint = OutPoint::new(tx_id.parse()?, output_index as u32);

            result.push(SpentTxo {
                outpoint,
                spending_tx: spending_tx_id.parse()?,
                spending_input_index: spending_input_index as u32,
            });
        }

        Ok(result)
    }
}

#[async_trait::async_trait]
impl chain::ChainRepository for ChainRepository {
    #[instrument(level = "trace", skip(self))]
    async fn add_block(
        &self,
        block: &BlockHeader,
        tx_outputs: &[AddressUtxo],
        tx_inputs: &[SpentTxo],
    ) -> Result<Vec<SpentTxo>, ChainRepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO blocks (block_hash, prev_block_hash, height)
               VALUES ($1, $2, $3)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(block.hash.to_string())
        .bind(block.prev.to_string())
        .bind(block.height as i64)
        .execute(&mut *tx)
        .await?;

        self.add_utxos(&mut tx, tx_outputs).await?;

        // NOTE: This also marks outputs as spent that were added in the current
        // transaction (as long as the default transaction isolation level is
        // not 'read uncommitted', which would be strange)
        let spent_txos = self.mark_spent(&mut tx, tx_inputs).await?;

        // correlate the transactions to the blocks
        let mut txns: Vec<_> = tx_outputs
            .iter()
            .map(|o| o.utxo.outpoint.txid.to_string())
            .chain(tx_inputs.iter().map(|i| i.spending_tx.to_string()))
            .collect();
        txns.dedup();
        let block_hashes: Vec<_> = txns.iter().map(|_| block.hash.to_string()).collect();
        sqlx::query(
            r#"INSERT INTO tx_blocks
               SELECT i.tx_id
               ,      i.block_hash
               FROM UNNEST(
                   $1::text[]
               ,   $2::text[]
               ) AS i (
                   tx_id
               ,   block_hash)
               WHERE EXISTS (SELECT 1
                             FROM tx_outputs o
                             WHERE o.tx_id = i.tx_id)
                   OR EXISTS (SELECT 1
                              FROM tx_inputs i
                              WHERE i.spending_tx_id = i.tx_id)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(&txns)
        .bind(&block_hashes)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(spent_txos)
    }

    #[instrument(level = "trace", skip(self))]
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

    #[instrument(level = "trace", skip(self))]
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

    #[instrument(level = "trace", skip(self))]
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

    #[instrument(level = "trace", skip(self))]
    async fn get_tip(&self) -> Result<Option<BlockHeader>, ChainRepositoryError> {
        let mut rows = sqlx::query(
            r#"SELECT block_hash
               ,      prev_block_hash
               ,      height
               FROM blocks
               WHERE height = (SELECT MAX(height) FROM blocks)"#,
        )
        .fetch(&*self.pool);

        let mut result: Option<BlockHeader> = None;
        while let Some(row) = rows.try_next().await? {
            if result.is_some() {
                return Err(ChainRepositoryError::MultipleTips);
            }

            let block_hash: String = row.try_get("block_hash")?;
            let prev_block_hash: String = row.try_get("prev_block_hash")?;
            let height: i64 = row.try_get("height")?;
            let header = BlockHeader {
                hash: block_hash.parse()?,
                prev: prev_block_hash.parse()?,
                height: height as u64,
            };
            result = Some(header);
        }
        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_txos_for_address(
        &self,
        address: &Address,
    ) -> Result<Vec<Txo>, ChainRepositoryError> {
        let mut rows = sqlx::query(
            r#"SELECT o.tx_id
            ,         o.output_index
            ,         o.amount
            ,         b.block_hash
            ,         b.height
            FROM tx_outputs o
            INNER JOIN tx_blocks tb ON tb.tx_id = o.tx_id
            INNER JOIN blocks b ON tb.block_hash = b.block_hash
            WHERE o.address = $1
            ORDER BY b.height, o.tx_id, o.output_index"#,
        )
        .bind(address.to_string())
        .fetch(&*self.pool);

        let mut result: Vec<Txo> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let txo = self.map_txo(address, &row).await?;
            result.push(txo);
        }
        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_txos_for_address_with_spends(
        &self,
        address: &Address,
    ) -> Result<Vec<TxoWithSpend>, ChainRepositoryError> {
        let mut rows = sqlx::query(
            r#"SELECT o.tx_id
            ,         o.output_index
            ,         o.amount
            ,         b.block_hash
            ,         b.height
            ,         sp.spending_tx_id
            ,         sp.spending_input_index
            ,         sp.spending_block_hash
            ,         sp.spending_block_height
            FROM tx_outputs o
            INNER JOIN tx_blocks tb ON tb.tx_id = o.tx_id
            INNER JOIN blocks b ON tb.block_hash = b.block_hash
            LEFT JOIN (
                SELECT i.tx_id
                ,      i.output_index
                ,      i.spending_tx_id
                ,      i.spending_input_index
                ,      ib.block_hash AS spending_block_hash
                ,      ib.height AS spending_block_height
                FROM tx_inputs i
                INNER JOIN tx_blocks itb ON itb.tx_id = i.spending_tx_id
                INNER JOIN blocks ib ON itb.block_hash = ib.block_hash
            ) sp ON sp.tx_id = o.tx_id AND sp.output_index = o.output_index
            WHERE o.address = $1
            ORDER BY b.height, o.tx_id, o.output_index"#,
        )
        .bind(address.to_string())
        .fetch(&*self.pool);

        let mut result = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let txo = self.map_txo(address, &row).await?;
            let spending_tx_id: Option<String> = row.try_get("spending_tx_id")?;
            let spend = match spending_tx_id {
                Some(spending_tx_id) => {
                    let spending_input_index: i64 = row.try_get("spending_input_index")?;
                    let block_hash: String = row.try_get("spending_block_hash")?;
                    let block_height: i64 = row.try_get("spending_block_height")?;
                    Some(TxoSpend {
                        spending_tx: spending_tx_id.parse()?,
                        spending_input_index: spending_input_index as u32,
                        block_hash: block_hash.parse()?,
                        block_height: block_height as u64,
                    })
                }
                None => None,
            };
            result.push(TxoWithSpend { txo, spend });
        }
        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_utxos(&self) -> Result<Vec<AddressUtxo>, ChainRepositoryError> {
        let mut rows = sqlx::query(
            r#"SELECT o.address
               ,      o.tx_id
               ,      o.output_index
               ,      o.amount
               ,      b.block_hash
               ,      b.height
               FROM tx_outputs o
               INNER JOIN tx_blocks tb ON tb.tx_id = o.tx_id
               INNER JOIN blocks b ON tb.block_hash = b.block_hash
               WHERE NOT EXISTS (SELECT 1 
                                 FROM tx_inputs i
                                 INNER JOIN tx_blocks itb ON itb.tx_id = i.spending_tx_id
                                 INNER JOIN blocks ib ON itb.block_hash = ib.block_hash
                                 WHERE o.tx_id = i.tx_id 
                                    AND o.output_index = i.output_index)
               ORDER BY o.address, b.height, o.tx_id, o.output_index"#,
        )
        .fetch(&*self.pool);

        let mut result: Vec<AddressUtxo> = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let address: String = row.try_get("address")?;
            let address = address
                .parse::<Address<NetworkUnchecked>>()?
                .require_network(self.network)?;
            let utxo = self.map_txo(&address, &row).await?;
            result.push(AddressUtxo { address, utxo });
        }
        Ok(result)
    }

    #[instrument(level = "trace", skip(self))]
    async fn undo_block(&self, hash: BlockHash) -> Result<(), ChainRepositoryError> {
        sqlx::query("DELETE FROM blocks WHERE block_hash = $1")
            .bind(hash.to_string())
            .execute(&*self.pool)
            .await?;
        Ok(())
    }
}

impl From<bitcoin::address::ParseError> for ChainRepositoryError {
    fn from(value: bitcoin::address::ParseError) -> Self {
        ChainRepositoryError::General(Box::new(value))
    }
}

impl From<bitcoin::hashes::hex::HexToArrayError> for ChainRepositoryError {
    fn from(value: bitcoin::hashes::hex::HexToArrayError) -> Self {
        ChainRepositoryError::General(Box::new(value))
    }
}

impl From<sqlx::Error> for ChainRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        ChainRepositoryError::General(Box::new(value))
    }
}
