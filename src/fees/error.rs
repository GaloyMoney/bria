use thiserror::Error;

#[derive(Debug, Error)]
pub enum FeeEstimationError {
    #[error("FeeEstimationError - FeeEstimation: {0}")]
    FeeEstimation(#[from] reqwest::Error),
}
