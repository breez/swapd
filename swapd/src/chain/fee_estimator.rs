use thiserror::Error;

#[derive(Debug)]
pub struct FeeEstimate {
    pub sat_per_kw: u32,
}

#[derive(Debug, Error)]
pub enum FeeEstimateError {
    #[error("unavailable")]
    Unavailable,
    #[error("{0}")]
    General(Box<dyn std::error::Error + Sync + Send>),
}

#[async_trait::async_trait]
pub trait FeeEstimator {
    async fn estimate_fee(&self, conf_target: i32) -> Result<FeeEstimate, FeeEstimateError>;
}
