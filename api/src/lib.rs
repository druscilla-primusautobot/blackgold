pub mod consts;
pub mod error;
pub mod event;
pub mod instruction;
pub mod sdk;
pub mod state;

pub mod prelude {
    pub use crate::consts::*;
    pub use crate::error::*;
    pub use crate::event::*;
    pub use crate::instruction::*;
    pub use crate::sdk::*;
    pub use crate::state::*;
}

use steel::*;

//& DRUSCILLA - Update this with the correct program id (Done) (Account) (Program_ID)
declare_id!("BGLDo13PkM4ZKJhQrb8WAda8wV86LHVCsFPKoy4ShQTi");
