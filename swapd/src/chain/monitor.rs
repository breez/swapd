use std::{collections::HashMap, sync::Arc, time::Duration};

use bitcoin::{block::Bip34Error, Address, Block, Network, OutPoint};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::chain::{AddressUtxo, ChainClient, ChainRepository, SpentTxo, Utxo};

use super::{memchain::Chain, types::BlockHeader, ChainError, ChainRepositoryError};

pub struct ChainMonitor<C, R>
where
    C: ChainClient,
    R: ChainRepository,
{
    network: Network,
    chain_client: Arc<C>,
    chain_repository: Arc<R>,
    poll_interval: Duration,
    full_sync_interval: Duration,
}

impl<C, R> ChainMonitor<C, R>
where
    C: ChainClient + Sync + Send + 'static,
    R: ChainRepository + Sync + Send + 'static,
{
    pub fn new(
        network: Network,
        chain_client: Arc<C>,
        chain_repository: Arc<R>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            chain_client,
            network,
            chain_repository,
            poll_interval,
            full_sync_interval: Duration::from_secs(60 * 60 * 24),
        }
    }

    pub async fn start(self: Arc<Self>, token: CancellationToken) -> Result<(), ChainError> {
        let blocks = self.chain_repository.get_block_headers().await?;
        let chain = match Chain::try_from(blocks) {
            Ok(chain) => chain,
            Err(e) => match e {
                ChainError::EmptyChain => {
                    // If the chain is empty, set the birthday to 20 blocks ago.
                    let tip_hash = self.chain_client.get_tip_hash().await?;
                    let mut birthday_header = self.chain_client.get_block_header(&tip_hash).await?;
                    for _n in 0..20 {
                        birthday_header = self
                            .chain_client
                            .get_block_header(&birthday_header.prev)
                            .await?;
                    }

                    self.chain_repository
                        .add_block(&birthday_header, &Vec::new(), &Vec::new())
                        .await?;
                    Chain::new(birthday_header)
                }
                _ => return Err(e),
            },
        };

        let birthday_chain = Chain::new(chain.base());
        let tracker = TaskTracker::new();
        let self1 = Arc::clone(&self);
        let self2 = Arc::clone(&self);
        let token1 = token.clone();
        let token2 = token.clone();
        tracker.spawn(async move {
            if let Err(e) = self1.start_tip_sync(chain, token1.child_token()).await {
                error!("chain tip sync exited with error: {:?}", e);
            }
            token1.cancel();
        });
        tracker.spawn(async move {
            if let Err(e) = self2
                .start_full_sync(birthday_chain, token2.child_token())
                .await
            {
                error!("chain full sync exited with error: {:?}", e);
            }
            token2.cancel();
        });
        tracker.wait().await;

        Ok(())
    }

    async fn start_full_sync(
        &self,
        birthday_chain: Chain,
        token: CancellationToken,
    ) -> Result<(), ChainError> {
        loop {
            if token.is_cancelled() {
                return Ok(());
            }

            let mut chain = birthday_chain.clone();
            self.do_sync(&mut chain, token.child_token()).await?;

            tokio::select! {
                _ = token.cancelled() => {
                    debug!("chain monitor full sync shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.full_sync_interval) => {}
            }
        }

        Ok(())
    }

    async fn start_tip_sync(
        &self,
        mut chain: Chain,
        token: CancellationToken,
    ) -> Result<(), ChainError> {
        loop {
            if token.is_cancelled() {
                return Ok(());
            }

            self.do_sync(&mut chain, token.child_token()).await?;

            tokio::select! {
                _ = token.cancelled() => {
                    debug!("chain monitor tip sync shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {}
            }
        }

        Ok(())
    }

    async fn do_sync(
        &self,
        existing_chain: &mut Chain,
        token: CancellationToken,
    ) -> Result<(), ChainError>
    where
        C: ChainClient,
        R: ChainRepository,
    {
        debug!(
            "chain sync starting from block {}",
            existing_chain.tip().hash
        );
        let tip_hash = self.chain_client.get_tip_hash().await?;
        let mut current_header = self.chain_client.get_block_header(&tip_hash).await?;
        let mut new_chain = Chain::new(current_header.clone());

        // Iterate backwards from the tip to get the missed block headers.
        loop {
            if token.is_cancelled() {
                return Ok(());
            }

            debug!(
                "got block header {}, height {}",
                current_header.hash, current_header.height
            );
            // Note that this is not checking the existing tip, because the chain
            // may have reorged.
            if existing_chain.contains_block(&current_header.hash) {
                break;
            }

            current_header = self
                .chain_client
                .get_block_header(&current_header.prev)
                .await?;
            new_chain.prepend(current_header.clone())?;
        }

        // If the base and tip don't match, there was a reorg.
        debug!(
            "block headers caught up. new chain base: {}, existing chain tip: {}",
            new_chain.base().hash,
            existing_chain.tip().hash
        );
        if new_chain.base() != existing_chain.tip() {
            for reorg_block in existing_chain.iter_backwards() {
                if token.is_cancelled() {
                    return Ok(());
                }

                if new_chain.contains_block(&reorg_block.hash) {
                    break;
                }

                debug!(
                    "block {} was reorged out of the chain, undoing block",
                    reorg_block.hash
                );
                self.chain_repository.undo_block(reorg_block.hash).await?;
            }
        }

        // Iterate forward from the last known block to the tip to process blocks.
        // Note that this always re-processes the last known block.
        for current_block in new_chain.iter_forwards() {
            if token.is_cancelled() {
                return Ok(());
            }

            debug!(
                "processing block {}, height {}",
                current_block.hash, current_block.height
            );
            let block = self.chain_client.get_block(&current_block.hash).await?;
            self.process_block(&block).await?;
        }

        existing_chain.retip(&new_chain)?;
        Ok(())
    }

    async fn process_block(&self, block: &Block) -> Result<(), ChainError> {
        // Check all transactions in the block
        // - does an output send to a known address?
        // - does an input spend a known utxo?
        let block_hash = block.block_hash();
        let prev_block_hash = block.header.prev_blockhash;
        let block_height = block.bip34_block_height()?;
        let mut spent_txos = Vec::new();
        let mut addresses = Vec::new();
        let mut address_utxos = HashMap::new();
        for tx in &block.txdata {
            let txid = tx.compute_txid();
            for (vout, output) in tx.output.iter().enumerate() {
                let address = match Address::from_script(&output.script_pubkey, self.network) {
                    Ok(address) => address,
                    Err(_) => continue,
                };
                addresses.push(address.clone());
                let entry = address_utxos.entry(address).or_insert(Vec::new());
                entry.push((OutPoint::new(txid, vout as u32), output.clone()));
            }

            for (vin, input) in tx.input.iter().enumerate() {
                spent_txos.push(SpentTxo {
                    spending_tx: txid,
                    spending_input_index: vin as u32,
                    outpoint: input.previous_output,
                });
            }
        }

        let watch_addresses = self
            .chain_repository
            .filter_watch_addresses(&addresses)
            .await?;
        let watch_utxos: Vec<AddressUtxo> = watch_addresses
            .into_iter()
            .filter_map(|a| {
                address_utxos.get(&a).map(|out| {
                    out.iter()
                        .map(|(outpoint, tx_out)| AddressUtxo {
                            address: a.clone(),
                            utxo: Utxo {
                                block_hash,
                                block_height,
                                outpoint: *outpoint,
                                tx_out: tx_out.clone(),
                            },
                        })
                        .collect()
                })
            })
            .flat_map(|a: Vec<AddressUtxo>| a)
            .collect();

        debug!(
            "block {} contains {} utxos to watched addresses",
            block_hash,
            watch_utxos.len()
        );
        self.chain_repository
            .add_block(
                &BlockHeader {
                    hash: block_hash,
                    height: block_height,
                    prev: prev_block_hash,
                },
                &watch_utxos,
                &spent_txos,
            )
            .await?;
        Ok(())
    }
}

impl From<ChainRepositoryError> for ChainError {
    fn from(value: ChainRepositoryError) -> Self {
        ChainError::Database(value)
    }
}

impl From<Bip34Error> for ChainError {
    fn from(value: Bip34Error) -> Self {
        ChainError::General(Box::new(value))
    }
}

mod tests {}
