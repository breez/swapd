use std::{future::Future, pin::pin, sync::Arc, time::Duration};

use futures::{future::FusedFuture, stream::FuturesUnordered, FutureExt, StreamExt};
use tracing::{debug, error};

use crate::{
    chain::ChainRepository,
    lightning::{LightningClient, PaymentResult},
    swap::SwapRepository,
};

pub struct PreimageMonitor<C, CR, SR>
where
    C: LightningClient,
    CR: ChainRepository,
    SR: SwapRepository,
{
    chain_repository: Arc<CR>,
    lightning_client: Arc<C>,
    poll_interval: Duration,
    swap_repository: Arc<SR>,
}

impl<C, CR, SR> PreimageMonitor<C, CR, SR>
where
    C: LightningClient,
    CR: ChainRepository,
    SR: SwapRepository,
{
    pub fn new(
        chain_repository: Arc<CR>,
        lightning_client: Arc<C>,
        poll_interval: Duration,
        swap_repository: Arc<SR>,
    ) -> Self {
        Self {
            chain_repository,
            lightning_client,
            poll_interval,
            swap_repository,
        }
    }

    pub async fn start<F: Future<Output = ()>>(
        &self,
        signal: F,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut sig = pin!(signal.fuse());
        if sig.is_terminated() {
            return Ok(());
        }

        loop {
            if let Err(e) = self.do_query_preimages().await {
                error!("failed to query preimages: {:?}", e);
            }

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

    async fn do_query_preimages(&self) -> Result<(), Box<dyn std::error::Error>> {
        let utxos = self.chain_repository.get_utxos().await?;
        let addresses: Vec<_> = utxos.iter().map(|u| u.address.clone()).collect();
        let hashes: Vec<_> = self
            .swap_repository
            .get_swaps(&addresses)
            .await?
            .into_iter()
            .filter(|swap| swap.1.preimage.is_none())
            .map(|swap| swap.1.swap.public.hash)
            .collect();

        let mut futures = FuturesUnordered::new();
        for hash in hashes {
            let fut = self.lightning_client.get_preimage(hash);
            futures.push(async move {
                let result = fut.await;
                (hash, result)
            });
        }

        while let Some((hash, result)) = futures.next().await {
            let maybe_preimage = match result {
                Ok(maybe_preimage) => maybe_preimage,
                Err(e) => {
                    error!(
                        "failed to query preimage for hash {} from lightning client: {:?}",
                        hash, e
                    );
                    continue;
                }
            };

            let preimage_result = match maybe_preimage {
                Some(preimage) => preimage,
                None => continue,
            };

            if let Err(e) = self
                .swap_repository
                .add_payment_result(
                    &hash,
                    &preimage_result.label,
                    &PaymentResult::Success {
                        preimage: preimage_result.preimage,
                    },
                )
                .await
            {
                error!(
                    "failed to insert preimage {} for hash {}, label {}: {:?}",
                    hex::encode(preimage_result.preimage),
                    hash,
                    preimage_result.label,
                    e
                );
                continue;
            }
        }

        Ok(())
    }
}
