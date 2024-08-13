use super::{FeeEstimate, FeeEstimateError, FeeEstimator};

pub struct WhatTheFeeEstimator {}

impl WhatTheFeeEstimator {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl FeeEstimator for WhatTheFeeEstimator {
    async fn estimate_fee(&self, _conf_target: i32) -> Result<FeeEstimate, FeeEstimateError> {
        todo!()
    }
}
