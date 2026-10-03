use thiserror::Error;

#[derive(Error, Debug)]
pub enum MintError {
    #[error("Mint API not implemented yet")]
    NotImplemented,
}
