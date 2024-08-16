use std::sync::Arc;

use bitcoin::OutPoint;

use crate::chain::{ChainClient, Utxo};

use super::ChainFilterRepository;

#[async_trait::async_trait]
pub trait ChainFilterService {
    async fn filter_utxos(&self, utxos: &[Utxo]) -> Result<Vec<Utxo>, Box<dyn std::error::Error>>;
}

#[derive(Debug)]
pub struct ChainFilterImpl<C, R>
where
    C: ChainClient,
    R: ChainFilterRepository,
{
    chain_client: Arc<C>,
    repository: Arc<R>,
}

impl<C, R> ChainFilterImpl<C, R>
where
    C: ChainClient,
    R: ChainFilterRepository,
{
    pub fn new(chain_client: Arc<C>, repository: Arc<R>) -> Self {
        Self {
            chain_client,
            repository,
        }
    }
}

#[async_trait::async_trait]
impl<C, R> ChainFilterService for ChainFilterImpl<C, R>
where
    C: ChainClient + Send + Sync,
    R: ChainFilterRepository + Send + Sync,
{
    async fn filter_utxos(&self, utxos: &[Utxo]) -> Result<Vec<Utxo>, Box<dyn std::error::Error>> {
        let outpoints: Vec<OutPoint> = utxos.iter().map(|u| u.outpoint).collect();
        let sender_addresses = self.chain_client.get_sender_addresses(&outpoints).await?;

        // TODO: Actually filter each individual utxo.
        let utxos = match self
            .repository
            .has_filtered_address(&sender_addresses)
            .await?
        {
            true => Vec::new(),
            false => utxos.to_vec(),
        };
        Ok(utxos)
    }
}
