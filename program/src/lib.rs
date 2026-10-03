mod automate;
mod bury;
mod buyback;
mod checkpoint;
mod claim_blackgold;
mod claim_sol;
mod close;
mod deploy;
mod log;
mod new_var;
mod reset;
mod update_protocol_config;
mod wrap;

use automate::*;
use bury::*;
use buyback::*;
use checkpoint::*;
use claim_blackgold::*;
use claim_sol::*;
use close::*;
use deploy::*;
use log::*;
use new_var::*;
use reset::*;
use update_protocol_config::*;
use wrap::*;

use blackgold_api::instruction::*;
use solana_security_txt::security_txt;
use steel::*;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let (ix, data) = parse_instruction(&blackgold_api::ID, program_id, data)?;

    match ix {
        // Miner
        BlackGoldInstruction::Automate => process_automate(accounts, data)?,
        BlackGoldInstruction::Checkpoint => process_checkpoint(accounts, data)?,
        BlackGoldInstruction::ClaimSOL => process_claim_sol(accounts, data)?,
        BlackGoldInstruction::ClaimBLACKGOLD => process_claim_blackgold(accounts, data)?,
        BlackGoldInstruction::Deploy => process_deploy(accounts, data)?,
        BlackGoldInstruction::Log => process_log(accounts, data)?,
        BlackGoldInstruction::Close => process_close(accounts, data)?,
        BlackGoldInstruction::Reset => process_reset(accounts, data)?,

        // Admin
        BlackGoldInstruction::Buyback => process_buyback(accounts, data)?,
        BlackGoldInstruction::Bury => process_bury(accounts, data)?,
        BlackGoldInstruction::Wrap => process_wrap(accounts, data)?,
        BlackGoldInstruction::NewVar => process_new_var(accounts, data)?,
        BlackGoldInstruction::UpdateProtocolConfig => {
            process_update_protocol_config(accounts, data)?
        }
        BlackGoldInstruction::Liq => return Err(ProgramError::InvalidInstructionData),
    }

    Ok(())
}

entrypoint!(process_instruction);

security_txt! {
    name: "BLACKGOLD",
    project_url: "https://blackgold.supply",
    contacts: "email:druscilla2024@gmail.com",
    policy: "https://github.com/druscilla-primusautobot/blackgold/blob/master/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/druscilla-primusautobot/blackgold"
}
