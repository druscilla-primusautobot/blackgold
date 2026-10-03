use thiserror::Error;

#[derive(Error, Debug)]
pub enum StakeError {
    #[error("Staking is not implemented yet")]
    NotImplemented,
}
