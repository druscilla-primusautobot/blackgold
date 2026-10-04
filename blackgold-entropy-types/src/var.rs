use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Var {
    pub authority: [u8; 32],
    pub provider: [u8; 32],

    pub commit: [u8; 32],
    pub seed: [u8; 32],
    pub slot_hash: [u8; 32],
    pub value: [u8; 32],

    pub samples: u64,     // always 1 for BlackGold
    pub start_at: u64,
    pub end_at: u64,

    pub is_auto: u64,
    pub id: u64,          // round_id
}
