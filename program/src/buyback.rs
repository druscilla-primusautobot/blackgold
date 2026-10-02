use blackgold_api::prelude::*;
use solana_program::log::sol_log;
use solana_program::native_token::lamports_to_sol;
use spl_token::amount_to_ui_amount;
use steel::*;

/// Swap vaulted SOL to ORE, and burn the ORE.
pub fn process_buyback(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    // Load accounts.
    let (blackgold_accounts, swap_accounts) = accounts.split_at(13);
    let [signer_info, board_info, _config_info, mint_info, treasury_info, treasury_blackgold_info, treasury_sol_info, stake_treasury_info, stake_treasury_blackgold_info, stake_vesting_info, token_program, blackgold_program, blackgold_stake_program] =
        blackgold_accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?.has_address(&BURY_AUTHORITY)?;
    board_info
        .has_address(&BOARD_ADDRESS)?
        .as_account_mut::<Board>(&blackgold_api::ID)?;
    let blackgold_mint = mint_info.has_address(&MINT_ADDRESS)?.as_mint()?;
    treasury_info
        .has_address(&TREASURY_ADDRESS)?
        .as_account_mut::<Treasury>(&blackgold_api::ID)?;
    let treasury_blackgold =
        treasury_blackgold_info.as_associated_token_account(treasury_info.key, &MINT_ADDRESS)?;
    treasury_sol_info.as_associated_token_account(treasury_info.key, &SOL_MINT)?;
    token_program.is_program(&spl_token::ID)?;
    blackgold_program.is_program(&blackgold_api::ID)?;
    blackgold_stake_program.is_program(&blackgold_stake_api::ID)?;

    // Sync native token balance.
    sync_native(treasury_sol_info)?;

    // Record pre-swap balances.
    let treasury_sol =
        treasury_sol_info.as_associated_token_account(treasury_info.key, &SOL_MINT)?;
    let pre_swap_blackgold_balance = treasury_blackgold.amount();
    let pre_swap_sol_balance = treasury_sol.amount();
    assert!(pre_swap_sol_balance > 0);

    // Record pre-swap mint supply.
    let pre_swap_mint_supply = blackgold_mint.supply();

    // Record pre-swap treasury lamports.
    let pre_swap_treasury_lamports = treasury_info.lamports();

    // Build swap accounts.
    let accounts: Vec<AccountMeta> = swap_accounts
        .iter()
        .map(|acc| {
            let is_signer = acc.key == treasury_info.key;
            AccountMeta {
                pubkey: *acc.key,
                is_signer,
                is_writable: acc.is_writable,
            }
        })
        .collect();

    // Build swap accounts infos.
    let accounts_infos: Vec<AccountInfo> = swap_accounts
        .iter()
        .map(|acc| AccountInfo { ..acc.clone() })
        .collect();

    // Invoke swap program.
    invoke_signed(
        &Instruction {
            program_id: SWAP_PROGRAM,
            accounts,
            data: data.to_vec(),
        },
        &accounts_infos,
        &blackgold_api::ID,
        &[TREASURY],
    )?;

    // Record post-swap treasury lamports.
    let post_swap_treasury_lamports = treasury_info.lamports();
    assert_eq!(
        post_swap_treasury_lamports, pre_swap_treasury_lamports,
        "Treasury lamports changed during swap: {} -> {}",
        pre_swap_treasury_lamports, post_swap_treasury_lamports
    );

    // Record post-swap mint supply.
    let post_swap_mint_supply = mint_info.as_mint()?.supply();
    assert_eq!(
        post_swap_mint_supply, pre_swap_mint_supply,
        "Mint supply changed during swap: {} -> {}",
        pre_swap_mint_supply, post_swap_mint_supply
    );

    // Record post-swap balances.
    let treasury_blackgold =
        treasury_blackgold_info.as_associated_token_account(treasury_info.key, &MINT_ADDRESS)?;
    let treasury_sol =
        treasury_sol_info.as_associated_token_account(treasury_info.key, &SOL_MINT)?;
    let post_swap_blackgold_balance = treasury_blackgold.amount();
    let _post_swap_sol_balance = treasury_sol.amount();
    let total_blackgold = post_swap_blackgold_balance - pre_swap_blackgold_balance;
    // assert_eq!(post_swap_sol_balance, 0);
    assert!(post_swap_blackgold_balance >= pre_swap_blackgold_balance);
    sol_log(
        &format!(
            "📈 Swapped {} SOL into {} BLACKGOLD",
            lamports_to_sol(pre_swap_sol_balance),
            amount_to_ui_amount(total_blackgold, TOKEN_DECIMALS),
        )
        .as_str(),
    );

    // Share some ORE with stakers.
    let shared_amount = total_blackgold / 10;
    if shared_amount > 0 {
        invoke_signed(
            &blackgold_stake_api::sdk::distribute(*treasury_info.key, shared_amount),
            &[
                treasury_info.clone(),
                treasury_blackgold_info.clone(),
                mint_info.clone(),
                stake_treasury_info.clone(),
                stake_treasury_blackgold_info.clone(),
                stake_vesting_info.clone(),
                token_program.clone(),
                blackgold_stake_program.clone(),
            ],
            &blackgold_api::ID,
            &[TREASURY],
        )?;
        sol_log(&format!(
            "💰 Shared {} BLACKGOLD",
            amount_to_ui_amount(shared_amount, TOKEN_DECIMALS)
        ));
    }

    // Burn ORE.
    let burn_amount = total_blackgold - shared_amount;
    burn_signed(
        treasury_blackgold_info,
        mint_info,
        treasury_info,
        token_program,
        burn_amount,
        &[TREASURY],
    )?;

    sol_log(
        &format!(
            "🔥 Buried {} BLACKGOLD",
            amount_to_ui_amount(burn_amount, TOKEN_DECIMALS)
        )
        .as_str(),
    );

    // Emit bury event.
    let mint = mint_info.as_mint()?;
    let ts = Clock::get()?.unix_timestamp;
    program_log(
        &[board_info.clone(), blackgold_program.clone()],
        BuryEvent {
            disc: 1,
            blackgold_buried: burn_amount,
            blackgold_shared: shared_amount,
            sol_amount: pre_swap_sol_balance,
            new_circulating_supply: mint.supply(),
            ts,
        }
        .to_bytes(),
    )?;

    Ok(())
}
