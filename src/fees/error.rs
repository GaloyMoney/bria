use thiserror::Error;

#[derive(Debug, Error)]
pub enum FeeEstimationError {
    #[error("FeeEstimationError - FeeEstimation: {0}")]
    FeeEstimation(#[from] reqwest_middleware::Error),
    #[error("FeeEstimationError - CouldNotDecodeResponseBody: {0}")]
    CouldNotDecodeResponseBody(reqwest::Error),
}
