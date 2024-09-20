use std::{collections::HashMap, future::Future, pin::pin, sync::Arc, time::Duration};

use bitcoin::{block::Bip34Error, Address, Block, BlockHash, Network, OutPoint};
use futures::future::{FusedFuture, FutureExt};
use tracing::{debug, field, info};

use crate::chain::{AddressUtxo, ChainClient, ChainRepository, SpentTxo, Utxo};

use super::{types::BlockHeader, ChainError, ChainRepositoryError};

pub struct ChainMonitor<C, R>
where
    C: ChainClient,
    R: ChainRepository,
{
    network: Network,
    chain_client: Arc<C>,
    chain_repository: Arc<R>,
    poll_interval: Duration,
}

impl<C, R> ChainMonitor<C, R>
where
    C: ChainClient,
    R: ChainRepository,
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
        }
    }

    pub async fn start<F: Future<Output = ()>>(&self, signal: F) -> Result<(), ChainError> {
        let blocks = self.chain_repository.get_block_headers().await?;
        let mut chain = match Chain::try_from(blocks) {
            Ok(chain) => chain,
            Err(e) => match e {
                ChainError::EmptyChain => {
                    // If the chain is empty, set the birthday to 20 blocks ago.
                    let tip_hash = self.chain_client.get_tip_hash().await?;
                    let mut birthday_header = self.chain_client.get_block_header(&tip_hash).await?;
                    for _n in 1..21 {
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

        let mut sig = pin!(signal.fuse());
        if sig.is_terminated() {
            return Ok(());
        }

        loop {
            chain = self.do_sync(&chain).await?;

            tokio::select! {
                _ = &mut sig => {
                    debug!("chain monitor shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {}
            }
        }

        Ok(())
    }

    async fn do_sync(&self, existing_chain: &Chain) -> Result<Chain, ChainError>
    where
        C: ChainClient,
        R: ChainRepository,
    {
        debug!("chain sync starting");
        let tip_hash = self.chain_client.get_tip_hash().await?;
        let mut current_header = self.chain_client.get_block_header(&tip_hash).await?;
        let mut new_chain = Chain::new(current_header.clone());

        // Iterate backwards from the tip to get the missed block headers.
        loop {
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
            new_chain.base, existing_chain.tip
        );
        if new_chain.base != existing_chain.tip {
            for reorg_block in existing_chain.iter_backwards() {
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
            debug!(
                "processing block {}, height {}",
                current_block.hash, current_block.height
            );
            let block = self.chain_client.get_block(&current_block.hash).await?;
            self.process_block(&block).await?;
        }

        new_chain.rebase(existing_chain)?;

        Ok(new_chain)
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
            let txid = tx.txid();
            for (vout, output) in tx.output.iter().enumerate() {
                let address = match Address::from_script(&output.script_pubkey, self.network) {
                    Ok(address) => address,
                    Err(_) => continue,
                };
                addresses.push(address.clone());
                let entry = address_utxos.entry(address).or_insert(Vec::new());
                entry.push((OutPoint::new(txid, vout as u32), output.value));
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
                        .map(|(outpoint, amount_sat)| AddressUtxo {
                            address: a.clone(),
                            utxo: Utxo {
                                amount_sat: *amount_sat,
                                block_hash,
                                block_height,
                                outpoint: *outpoint,
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

#[derive(Clone)]
struct Chain {
    tip: BlockHash,
    base: BlockHash,
    blocks: HashMap<BlockHash, BlockInfo>,
}

#[derive(Clone)]
struct BlockInfo {
    header: BlockHeader,
    next: Option<BlockHash>,
}

impl Chain {
    fn new(tip: BlockHeader) -> Self {
        let mut chain = Chain {
            tip: tip.hash,
            base: tip.hash,
            blocks: HashMap::new(),
        };
        chain.blocks.insert(
            tip.hash,
            BlockInfo {
                header: tip,
                next: None,
            },
        );
        chain
    }

    fn contains_block(&self, hash: &BlockHash) -> bool {
        self.blocks.contains_key(hash)
    }

    fn get_block(&self, hash: &BlockHash) -> Result<BlockHeader, ChainError> {
        match self.blocks.get(hash) {
            Some(block) => Ok(block.header.clone()),
            None => Err(ChainError::BlockNotFound),
        }
    }

    fn prepend(&mut self, base: BlockHeader) -> Result<(), ChainError> {
        let old_base = self
            .blocks
            .get(&self.base)
            .expect("chain doesn't contain its own base");
        if old_base.header.prev != base.hash {
            return Err(ChainError::InvalidChain);
        }

        self.base = base.hash;
        self.blocks.insert(
            base.hash,
            BlockInfo {
                header: base,
                next: Some(old_base.header.hash),
            },
        );
        Ok(())
    }

    fn rebase(&mut self, other: &Chain) -> Result<(), ChainError> {
        let mut next_block = self
            .blocks
            .get(&self.base)
            .expect("chain doesn't contain its own base")
            .header
            .clone();
        loop {
            if next_block.hash == other.base {
                break;
            }

            let current_block = other.get_block(&next_block.prev)?;
            self.prepend(current_block.clone())?;
            next_block = current_block;
        }

        Ok(())
    }

    fn iter_forwards(&self) -> ForwardChainIterator {
        ForwardChainIterator::new(self)
    }

    fn iter_backwards(&self) -> BackwardChainIterator {
        BackwardChainIterator::new(self)
    }
}

struct ForwardChainIterator<'a> {
    chain: &'a Chain,
    current: Option<&'a BlockInfo>,
}

impl<'a> ForwardChainIterator<'a> {
    fn new(chain: &'a Chain) -> Self {
        let current = chain
            .blocks
            .get(&chain.base)
            .expect("chain does not contain its own base");
        ForwardChainIterator {
            chain,
            current: Some(current),
        }
    }
}

impl<'a> Iterator for ForwardChainIterator<'a> {
    type Item = &'a BlockHeader;
    fn next(&mut self) -> Option<&'a BlockHeader> {
        let current = match self.current {
            Some(current) => current,
            None => return None,
        };

        self.current = match current.next {
            Some(next) => Some(
                self.chain
                    .blocks
                    .get(&next)
                    .expect("chain does not contain expected next block"),
            ),
            None => None,
        };

        Some(&current.header)
    }
}

struct BackwardChainIterator<'a> {
    chain: &'a Chain,
    current: Option<&'a BlockInfo>,
}

impl<'a> BackwardChainIterator<'a> {
    fn new(chain: &'a Chain) -> Self {
        let current = chain
            .blocks
            .get(&chain.tip)
            .expect("chain doesn't contain its own tip");
        BackwardChainIterator {
            chain,
            current: Some(current),
        }
    }
}

impl<'a> Iterator for BackwardChainIterator<'a> {
    type Item = &'a BlockHeader;
    fn next(&mut self) -> Option<&'a BlockHeader> {
        let current = match self.current {
            Some(current) => current,
            None => return None,
        };

        self.current = match current.header.hash == self.chain.base {
            true => None,
            false => Some(
                self.chain
                    .blocks
                    .get(&current.header.prev)
                    .expect("chain does not contain expected prev block"),
            ),
        };

        Some(&current.header)
    }
}

impl TryFrom<Vec<super::types::BlockHeader>> for Chain {
    type Error = ChainError;

    fn try_from(headers: Vec<super::types::BlockHeader>) -> Result<Self, Self::Error> {
        if headers.is_empty() {
            return Err(ChainError::EmptyChain);
        }

        let tip_header = headers[0].clone();
        let mut chain = Chain::new(tip_header);
        for header in headers.into_iter().skip(1) {
            chain.prepend(header)?;
        }
        Ok(chain)
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
