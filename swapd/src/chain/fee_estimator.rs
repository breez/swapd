#[derive(Debug)]
pub struct FeeEstimate {
    pub sat_per_kw: u32,
}

#[derive(Debug)]
pub enum FeeEstimateError {
    Unavailable,
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[async_trait::async_trait]
pub trait FeeEstimator {
    async fn estimate_fee(&self, conf_target: i32) -> Result<FeeEstimate, FeeEstimateError>;
}
