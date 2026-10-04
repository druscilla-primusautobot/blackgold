use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct Open {
    pub id: [u8; 8],
    pub commit: [u8; 32],
    pub samples: [u8; 8],
    pub end_at: [u8; 8],
    pub is_auto: [u8; 8],
}
