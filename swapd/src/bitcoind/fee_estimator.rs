use std::sync::Arc;

use crate::chain::{self, FeeEstimate, FeeEstimateError};

use super::{client::CallError, BitcoindClient};

#[derive(Debug)]
pub struct FeeEstimator {
    client: Arc<BitcoindClient>,
}

impl FeeEstimator {
    pub fn new(client: Arc<BitcoindClient>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl chain::FeeEstimator for FeeEstimator {
    async fn estimate_fee(&self, conf_target: i32) -> Result<FeeEstimate, FeeEstimateError> {
        let target = conf_target.clamp(1, 1008);
        let fee = self.client.estimatesmartfee(target as u32).await?;
        // feerate is btc/kb (multiply by 100_000_000 and divide by 4)
        let sat_per_kw = (fee.feerate * 25_000_000.0).ceil() as u32;

        Ok(FeeEstimate { sat_per_kw })
    }
}

impl From<CallError> for FeeEstimateError {
    fn from(value: CallError) -> Self {
        FeeEstimateError::General(Box::new(value))
    }
}
