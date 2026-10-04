use num_enum::{IntoPrimitive, TryFromPrimitive};

#[repr(u8)]
#[derive(Clone, Copy, Debug, IntoPrimitive, TryFromPrimitive)]
pub enum BlackgoldEntropyInstruction {
    Open = 0,
    Sample = 1,
    Reveal = 2,
    Next = 3,
    Close = 4,
}
