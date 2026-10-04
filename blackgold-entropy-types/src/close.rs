use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct Close {
    pub id: [u8; 8],
}
