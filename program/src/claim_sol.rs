use blackgold_api::prelude::*;
use blackgold_api::sdk::program_log;
use solana_program::{log::sol_log, native_token::lamports_to_sol};
use steel::*;

/// Claims a block reward.
pub fn process_claim_sol(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
    // Load accounts.
    let clock = Clock::get()?;
    let [signer_info, board_info, miner_info, system_program, blackgold_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    board_info.has_address(&BOARD_ADDRESS)?;
    let miner = miner_info
        .has_seeds(&[MINER, &signer_info.key.to_bytes()], &blackgold_api::ID)?
        .as_account_mut::<Miner>(&blackgold_api::ID)?
        .assert_mut(|m| m.authority == *signer_info.key)?;
    system_program.is_program(&system_program::ID)?;
    blackgold_program.is_program(&blackgold_api::ID)?;

    // Normalize amount.
    let amount = miner.claim_sol(&clock);

    sol_log(&format!("Claiming {} SOL", lamports_to_sol(amount)).as_str());

    // Transfer reward to recipient.
    miner_info.send(amount, signer_info);

    // Emit claim event.
    program_log(
        &[board_info.clone(), blackgold_program.clone()],
        ClaimEvent {
            disc: 4,
            authority: miner.authority,
            amount,
            claim_type: 0, // SOL
            ts: clock.unix_timestamp,
        }
        .to_bytes(),
    )?;

    Ok(())
}
