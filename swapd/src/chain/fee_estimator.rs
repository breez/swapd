use thiserror::Error;
use tracing::error;

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

#[derive(Debug)]
pub struct FallbackFeeEstimator<E1, E2>
where
    E1: FeeEstimator,
    E2: FeeEstimator,
{
    estimator1: E1,
    estimator2: E2,
}

impl<E1, E2> FallbackFeeEstimator<E1, E2>
where
    E1: FeeEstimator,
    E2: FeeEstimator,
{
    pub fn new(estimator1: E1, estimator2: E2) -> Self {
        Self {
            estimator1,
            estimator2,
        }
    }
}

#[async_trait::async_trait]
impl<E1, E2> FeeEstimator for FallbackFeeEstimator<E1, E2>
where
    E1: FeeEstimator + Send + Sync,
    E2: FeeEstimator + Send + Sync,
{
    async fn estimate_fee(&self, conf_target: i32) -> Result<FeeEstimate, FeeEstimateError> {
        match self.estimator1.estimate_fee(conf_target).await {
            Ok(res) => return Ok(res),
            Err(e) => {
                error!("fee estimator 1 returned error: {:?}", e)
            }
        }

        match self.estimator2.estimate_fee(conf_target).await {
            Ok(res) => return Ok(res),
            Err(e) => {
                error!("fee estimator 2 returned error: {:?}", e)
            }
        }

        Err(FeeEstimateError::Unavailable)
    }
}
