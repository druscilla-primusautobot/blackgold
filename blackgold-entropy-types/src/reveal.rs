use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct Reveal {
    pub id: [u8; 8],
    pub seed: [u8; 32],
}
