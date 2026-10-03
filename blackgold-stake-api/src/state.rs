use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StakePlaceholder {
    pub initialized: bool,
}
