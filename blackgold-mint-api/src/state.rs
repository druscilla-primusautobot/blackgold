use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MintPlaceholder {
    pub initialized: bool,
}
