use blackgold_api::prelude::*;
use entropy_api::state::Var;
use solana_program::{keccak::hashv, log::sol_log, native_token::lamports_to_sol};
use steel::*;

//* */ This is the handler called when BlackGoldInstruction::Deploy is dispatched from lib.rs.
/// Deploys capital to prospect on a square.
pub fn process_deploy(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    // Parse data.
    //* Decodes the raw instruction data into our Deploy struct.
    let args = Deploy::try_from_bytes(data)?;

    //* Converts the amount from a byte array to a u64 integer.
    //* lamports per square to deploy.
    let mut amount = u64::from_le_bytes(args.amount);

    //* Converts the mask from a byte array to a u32 integer.
    //* 32‑bit bitmask of squares the user wants to deploy to (25 used, 7 unused).
    let mask = u32::from_le_bytes(args.squares);

    // Load accounts.
    //* Clock::get(): reads current slot/time.
    let clock = Clock::get()?;

    //* Splits the accounts slice into two parts: blackgold_accounts and entropy_accounts.
    //* first 10: core BlackGold accounts
    //* remaining: entropy accounts (var and entropy program)
    let (blackgold_accounts, entropy_accounts) = accounts.split_at(10);

    //* Logs counts for debugging.
    sol_log(&format!("BlackGold accounts: {:?}", blackgold_accounts.len()).to_string());
    sol_log(&format!("Entropy accounts: {:?}", entropy_accounts.len()).to_string());

    //* Destructure BlackGold accounts from the "blackgold_accounts" slice into individual account variables.
    //* Enforces exactly 10 BlackGold accounts in the expected order.
    let [signer_info, authority_info, automation_info, board_info, config_info, miner_info, round_info, treasury_info, system_program, blackgold_program] =
        blackgold_accounts
    else {
        //* If the number of BlackGold accounts is not exactly 10, return an error.
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    //* Validates the signer account is a signer of the transaction. (transaction must be signed by this key.)

    signer_info.is_signer()?;
    //* Validates the authority account is writable. (transaction must be able to modify this account.) (authority account must be writable.)
    authority_info.is_writable()?;

    //* automation_info.has_seeds: automation PDA must be derived from:
    //* seed: AUTOMATION (our b"auto")
    //* authority pubkey (authority account's public key)
    //* program ID: blackgold_api::ID
    //*  This ensures the automation account is the correct PDA.

    automation_info.is_writable()?.has_seeds(
        &[AUTOMATION, &authority_info.key.to_bytes()],
        &blackgold_api::ID,
    )?;

    //* Load config, board, round, treasury, miner, system program
    //* Ensures config_info is the singleton config account and deserializes it.
    let config = config_info
        .has_address(&CONFIG_ADDRESS)?
        .as_account::<Config>(&blackgold_api::ID)?;

    //* Ensures board_info is the board PDA.
    //* Asserts current slot is within the active round window.
    //* Ensures board_info is the board PDA and deserializes it.
    let board = board_info
        .has_address(&BOARD_ADDRESS)?
        .as_account_mut::<Board>(&blackgold_api::ID)?
        .assert_mut(|b| clock.slot >= b.start_slot && clock.slot < b.end_slot)?;

    //* Ensures round_info is the round PDA for board.round_id.
    //* Asserts round’s id matches board’s round_id.
    //* Ensures round_info is the round PDA for board.round_id and deserializes it.
    let round = round_info
        .has_seeds(&[ROUND, &board.round_id.to_le_bytes()], &blackgold_api::ID)?
        .as_account_mut::<Round>(&blackgold_api::ID)?
        .assert_mut(|r| r.id == board.round_id)?;

    //* Ensures treasury_info is the singleton treasury account.
    //* Ensures treasury_info is the treasury PDA and deserializes it.
    let treasury = treasury_info
        .has_address(&TREASURY_ADDRESS)?
        .as_account_mut::<Treasury>(&blackgold_api::ID)?;

    //* Ensures miner_info is the miner PDA for the given authority.
    miner_info
        .is_writable()?
        .has_seeds(&[MINER, &authority_info.key.to_bytes()], &blackgold_api::ID)?;

    //* Confirms system program account is correct.
    system_program.is_program(&system_program::ID)?;

    // Wait until first deploy to start round.

    //& Start round if first deploy.
    //* If end_slot is u64::MAX, the round hasn’t started yet.
    //* Sets:
    //* board.start_slot to current slot.
    //* board.end_slot to start + configured round length.
    //*  round.expires_at to end + one day (grace period).
    if board.end_slot == u64::MAX {
        board.start_slot = clock.slot;
        board.end_slot = board.start_slot + config.protocol.round_slots;
        round.expires_at = board.end_slot + ONE_DAY_SLOTS;

        // Bump var to the next value.
        //* Expects 2 entropy accounts:
        //* var_info: the randomness var account.
        //* entropy_program: the entropy program itself.
        let [var_info, entropy_program] = entropy_accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        //* Validates:
        //* var_info has the correct VAR_ADDRESS.
        //* Var.authority matches the board PDA.
        //* entropy_program is the correct program.
        var_info
            .has_address(&VAR_ADDRESS)?
            .as_account::<Var>(&entropy_api::ID)?
            .assert(|v| v.authority == *board_info.key)?;
        entropy_program.is_program(&entropy_api::ID)?;

        // Bump var to the next value.
        //* Calls entropy_api::sdk::next to bump the var to the next randomness value, keyed by board.end_slot.
        //* Signs with the BOARD PDA seed.
        //* This is the “advance randomness” step for the round.
        //* This whole block is the “round start + randomness bump” logic.
        invoke_signed(
            &entropy_api::sdk::next(*board_info.key, *var_info.key, board.end_slot),
            &[board_info.clone(), var_info.clone()],
            &entropy_api::ID,
            &[BOARD],
        )?;
    }

    // Get the automation.

    //& Load automation (if present)

    let mut strategy = u64::MAX;

    //* If automation_info has data, it’s an existing automation account.
    let automation = if !automation_info.data_is_empty() {
        //* Validates:
        //* executor is either the signer or a special EXECUTOR_ADDRESS.
        //* authority matches authority_info.
        let automation = automation_info
            .as_account_mut::<Automation>(&blackgold_api::ID)?
            .assert_mut(|a| a.executor == *signer_info.key || a.executor == EXECUTOR_ADDRESS)?
            .assert_mut(|a| a.authority == *authority_info.key)?;

        // Conditional deploy.
        //* Conditional deploy: if treasury’s motherlode is outside the configured range, skip deploy.
        let max_motherlode = automation.conditions.max_motherlode as u64 * ONE_BLACKGOLD;
        let min_motherlode = automation.conditions.min_motherlode as u64 * ONE_BLACKGOLD;
        if treasury.motherlode > max_motherlode || treasury.motherlode < min_motherlode {
            return Ok(());
        }

        // Set strategy.
        strategy = automation.strategy as u64;
        // Wraps automation in Some or None.
        Some(automation)
    } else {
        //* If automation_info is empty, there’s no automation account. Set automation to None.
        None
    };

    // Update amount and mask for automation.
    //& Build squares array (mask) based on automation strategy
    //* Initializes a 25‑element boolean array for squares.
    let mut squares = [false; 25];
    //* If automation exists, chooses behavior based on strategy.
    if let Some(automation) = &automation {
        // Set amount and squares based on automation strategy.
        match AutomationStrategy::from_u64(automation.strategy as u64) {
            //& Preferred strategy
            //* Uses automation’s amount and mask directly.
            AutomationStrategy::Preferred => {
                // Preferred automation strategy. Use the miner authority's provided mask.
                amount = automation.amount;
                for i in 0..25 {
                    squares[i] = (automation.mask & (1 << i)) != 0;
                }
            }
            //& Random strategy
            //* Generates a random mask based on the number of squares the user wants to deploy to.
            //* Random strategy (with solo/split preferences)
            AutomationStrategy::Random => {
                // Random automation strategy. Generate a random mask based on number of squares user wants to deploy to.
                amount = automation.amount;

                // If first deploy, use the mask provided by the user.
                //* First deploy: use automation’s mask.
                if automation.total_sol_spent == 0 {
                    for i in 0..25 {
                        squares[i] = (automation.mask & (1 << i)) != 0;
                    }
                } else if automation.conditions.solo_tiles > 0
                    || automation.conditions.split_tiles > 0
                {
                    // User has a preferred solo / split strategy. Generate a mask based on this preferrence.

                    // First generate the solo / split mask.
                    let distribution_mask = round.distribution_mask();

                    // Build squares array based on user's preferred solo and split tiles using fixed-size arrays.

                    // First collect the indices of solos and splits (maximum 25 of each).
                    // * Collect indices of solo vs split tiles.
                    let mut solo_idxs = [0usize; 25];
                    let mut split_idxs = [0usize; 25];
                    let mut num_solo = 0usize;
                    let mut num_split = 0usize;
                    for i in 0..25 {
                        if (distribution_mask & (1 << i)) != 0 {
                            solo_idxs[num_solo] = i;
                            num_solo += 1;
                        } else {
                            split_idxs[num_split] = i;
                            num_split += 1;
                        }
                    }

                    //* Desired counts of solo/split tiles.
                    let solo_pref = automation.conditions.solo_tiles as usize;
                    let split_pref = automation.conditions.split_tiles as usize;

                    // Build a random seed using user and round id for deterministic shuffle
                    //* Builds a deterministic seed from authority + round id + tag.
                    let mut seed = [0u8; 32];
                    let h = hashv(&[
                        &automation.authority.to_bytes(),
                        &round.id.to_le_bytes(),
                        b"solo_split_mask",
                    ])
                    .0;
                    seed.copy_from_slice(&h);

                    // Deterministic shuffle for solo and split index arrays in place
                    //* Fisher–Yates‑style deterministic shuffle using the seed.
                    // This ensures the same seed always produces the same shuffle.
                    fn deterministic_shuffle(idxs: &mut [usize], len: usize, seed: &[u8; 32]) {
                        let mut local_seed = [0u8; 8];
                        for i in (1..len).rev() {
                            // Use 8 bytes from the 32-byte seed cycling per index
                            let start = (i % (32 - 8 + 1)) as usize;
                            local_seed.copy_from_slice(&seed[start..start + 8]);
                            let hashval = u64::from_le_bytes(local_seed);
                            let j = (hashval % (i as u64 + 1)) as usize;
                            // Swap i <-> j
                            let tmp = idxs[i];
                            idxs[i] = idxs[j];
                            idxs[j] = tmp;
                        }
                    }

                    // Apply shuffle to both solo and split slice parts
                    //* Shuffles solo and split indices.
                    //* Split the slices to only include the valid indices (num_solo and num_split).
                    let (solo_idxs_slice, split_idxs_slice) =
                        (&mut solo_idxs[..num_solo], &mut split_idxs[..num_split]);
                    deterministic_shuffle(solo_idxs_slice, num_solo, &seed);
                    deterministic_shuffle(split_idxs_slice, num_split, &seed);

                    // Prepare the tiles to select
                    //* Clears squares and sets chosen solo/split tiles to true.
                    //* Calculates how many solo/split tiles to select based on user preference and available tiles.
                    let num_solo_to_select = num_solo.min(solo_pref);
                    let num_split_to_select = num_split.min(split_pref);

                    // Clear current squares
                    //* Clears the squares array to false.
                    for i in 0..25 {
                        squares[i] = false;
                    }

                    // Set true on chosen solo tiles
                    //* Sets the first num_solo_to_select shuffled solo indices to true in squares.
                    for n in 0..num_solo_to_select {
                        squares[solo_idxs[n]] = true;
                    }

                    // Set true on chosen split tiles
                    //* Sets the first num_split_to_select shuffled split indices to true in squares.
                    for n in 0..num_split_to_select {
                        squares[split_idxs[n]] = true;
                    }
                } else {
                    // If not first deploy and no preferred solo / split strategy, generate a random mask based on number of squares user wants to deploy to.
                    //* Counts how many squares user wants.
                    //* Builds a deterministic random mask using generate_random_mask.
                    let num_squares = (0..25)
                        .filter(|i| (automation.mask & (1 << i)) != 0)
                        .count() as u64;
                    let r = hashv(&[&automation.authority.to_bytes(), &round.id.to_le_bytes()]).0;
                    squares = generate_random_mask(num_squares, &r);
                }
            }

            //& Discretionary strategies
            //* Uses executor’s provided mask.
            //* Caps amount by automation’s configured amount.
            AutomationStrategy::Discretionary | AutomationStrategy::DiscretionaryBps => {
                // Discretionary automation strategy. Use the executor's provided mask.
                amount = amount.min(automation.amount);
                for i in 0..25 {
                    squares[i] = (mask & (1 << i)) != 0;
                }
            }
        }
    } else {
        //& No automation: use the user’s provided mask.
        // Convert provided 32-bit mask into array of 25 booleans, where each bit in the mask
        // determines if that square index is selected (true) or not (false)
        //* If no automation, use the user’s mask directly.
        //* Convert the provided 32-bit mask into an array of 25 booleans, where each bit in the mask determines if that square index is selected (true) or not (false).
        //* For each square index 0..25, check if the corresponding bit in the mask
        for i in 0..25 {
            squares[i] = (mask & (1 << i)) != 0;
        }
    }

    //& Open or load miner account
    // Open miner account.
    //* If miner PDA is empty, create it using:
    //* seed: MINER (b"driller")
    //* signer’s pubkey.
    let miner = if miner_info.data_is_empty() {
        create_program_account::<Miner>(
            miner_info,
            system_program,
            signer_info,
            &blackgold_api::ID,
            &[MINER, &signer_info.key.to_bytes()],
        )?;

        //* Initializes miner fields.
        let miner = miner_info.as_account_mut::<Miner>(&blackgold_api::ID)?;
        miner.authority = *signer_info.key;
        miner.deployed = [0; 25];
        miner.cumulative = [0; 25];
        miner.rewards_sol = 0;
        miner.rewards_blackgold = 0;
        miner.round_id = 0;
        miner.checkpoint_id = 0;
        miner.lifetime_rewards_sol = 0;
        miner.lifetime_rewards_blackgold = 0;
        miner.auto_return = 1;
        miner
    } else {
        //* If miner exists, ensures:
        //* authority matches automation’s authority (if automation),
        //* otherwise matches signer.
        miner_info
            .as_account_mut::<Miner>(&blackgold_api::ID)?
            .assert_mut(|m| {
                //* If automation exists, authority must match automation’s authority.
                if let Some(automation) = &automation {
                    m.authority == automation.authority
                } else {
                    //* If no automation, authority must match signer.
                    m.authority == *signer_info.key
                }
            })?
    };

    //& Reset miner for new round
    // Reset miner
    //* If miner is on a different round than current:
    if miner.round_id != round.id {
        // Assert miner has checkpointed prior round.
        //* Assert they checkpointed the previous round. (If not, they forfeit rewards.)
        assert!(
            miner.checkpoint_id == miner.round_id,
            "Miner has not checkpointed"
        );

        // Reset miner for new round.
        //* Reset miner’s deployed squares.
        //* Set cumulative to current round’s deployed.
        //* Update miner’s round id.
        miner.deployed = [0; 25];
        miner.cumulative = round.deployed;
        miner.round_id = round.id;
    }

    //& First deploy flag
    // Update total miners for round.
    //* If this is the miner’s first deploy in this round, increment round.total_miners.
    let is_first_deploy = miner.deployed.iter().sum::<u64>() == 0;

    //& Automation balance check on first deploy
    // Close automation if it doesn't have enough balance to cover all requested squares.
    //* On first deploy:
    //* compute total SOL needed + fee.
    //* if automation balance is insufficient:
    //* send fee to signer.
    //* close automation account to authority.
    //* exit early.
    if is_first_deploy {
        if let Some(automation) = &automation {
            let required_squares = squares.iter().filter(|&&s| s).count() as u64;
            let total_deploy = amount * required_squares;
            let estimated_fee = automation.min_fee(total_deploy);
            if automation.balance < total_deploy + estimated_fee {
                automation_info.send(estimated_fee, &signer_info);
                automation_info.close(authority_info)?;
                return Ok(());
            }
        }
    }

    //& Deploy to squares
    // Calculate all deployments.
    //* Iterate over squares:
    //* skip out‑of‑range indices.
    //* skip if not selected.
    //* skip if miner already deployed there.
    let mut total_amount = 0;
    let mut total_squares = 0;
    let mut deployed_squares = [false; 25];
    for (square_id, &should_deploy) in squares.iter().enumerate() {
        // Skip if square index is out of bounds.
        if square_id > 24 {
            break;
        }

        // Skip if square is not deployed to.
        if !should_deploy {
            continue;
        }

        // Skip if miner already deployed to this square.
        if miner.deployed[square_id] > 0 {
            continue;
        }

        // Record cumulative amount.
        //* Set miner’s cumulative to current round deployed.
        miner.cumulative[square_id] = round.deployed[square_id];

        // Update miner
        //* Set miner’s deployed amount.
        miner.deployed[square_id] = amount;

        // Update board
        //* Increase round’s deployed and count for that square.
        round.deployed[square_id] += amount;
        // round.total_deployed += amount;
        round.count[square_id] += 1;

        // Update totals.
        //* Track totals and mark deployed squares.
        //* Outside the loop vars - for total_amount, total_squares, and deployed_squares. for each square deployed to, add amount to total_amount, increment total_squares, and mark deployed_squares[square_id] = true.
        total_amount += amount;
        total_squares += 1;
        deployed_squares[square_id] = true;
    }

    //& Update total miners for round
    // Update total miners for round.
    //* If this is the miner’s first deploy in this round and they deployed to at least one square, increment round.total_miners.
    if is_first_deploy && total_amount > 0 {
        round.total_miners += 1;
    }

    //& Increment miner lifetime deployed
    // Increment miner lifetime deployed.
    //* Tracks miner’s total deployed over lifetime.
    //* Increment miner’s lifetime deployed by adding total_amount.
    miner.lifetime_deployed += total_amount;

    // Top up checkpoint fee.
    //* If miner hasn’t paid checkpoint fee yet:
    //* set fee.
    //* collect it from signer.
    if miner.checkpoint_fee == 0 {
        miner.checkpoint_fee = CHECKPOINT_FEE;
        miner_info.collect(CHECKPOINT_FEE, &signer_info)?;
    }

    //& Transfer SOL (automation vs manual)
    // Transfer SOL.
    //* Automation path:
    //* update total SOL spent.
    if let Some(automation) = automation {
        // Update automation total sol spent.
        //* Increment automation.total_sol_spent by total_amount. (add total_amount to automation.total_sol_spent)
        automation.total_sol_spent += total_amount;

        // Calculate automation fee.
        //* If first deploy and total_amount > 0, compute automation fee using automation.min_fee(total_amount).
        //* Otherwise, fee is 0.
        let automation_fee = if is_first_deploy && total_amount > 0 {
            automation.min_fee(total_amount)
        } else {
            //* Fee only on first deploy.
            //* If not first deploy, fee is 0.
            0
        };

        // Update automation balance.
        //* Deduct from automation balance.
        automation.balance -= total_amount + automation_fee;
        //* Send deployed SOL to round.
        automation_info.send(total_amount, &round_info);
        //* Send fee to signer.
        automation_info.send(automation_fee, &signer_info);

        // Close automation if balance is less than what's required to deploy 1 square.
        //* If automation.balance < automation.amount + automation.min_fee(automation.amount), close automation.
        if automation.balance < automation.amount + automation.min_fee(automation.amount) {
            //* Close automation account to authority if balance is insufficient to deploy at least one square.
            automation_info.close(authority_info)?;
        }
    } else {
        //* Manual path: collect total_amount from signer.
        //* If no automation, collect total_amount from signer.
        //* This is the manual deploy path: collect total_amount from signer.
        round_info.collect(total_amount, &signer_info)?;
    }

    // Rebuild the mask from the deployed squares.
    //* Build a bitmask of squares actually deployed to (may differ from requested mask due to skips)
    let mut deployed_mask = 0;
    for (square_id, &deployed) in deployed_squares.iter().enumerate() {
        if deployed {
            deployed_mask |= 1 << square_id;
        }
    }

    // Log the deploy event.
    //* Logs a structured event with:
    //* miner authority
    //* amount per square
    //* deployed mask
    //* round id
    //* signer
    //* strategy
    //* total squares
    //* timestamp
    //* This is what our indexer / dashboard will consume.
    program_log(
        &[board_info.clone(), blackgold_program.clone()],
        DeployEvent {
            disc: 2,
            authority: miner.authority,
            amount,
            mask: deployed_mask as u64,
            round_id: round.id,
            signer: *signer_info.key,
            strategy,
            total_squares,
            ts: clock.unix_timestamp,
        }
        .to_bytes(),
    )?;

    // Log
    //* Simple log for explorers / debugging.
    sol_log(
        &format!(
            "Round #{}: deploying {} SOL to {} squares",
            round.id,
            lamports_to_sol(amount),
            total_squares,
        )
        .as_str(),
    );

    Ok(())
}

//* Helper:
//* Greedy selection algorithm:
//* walks through 25 positions.
//* uses rand_byte and remaining_needed/positions to decide whether to set a square.
//* ensures exactly num_squares (or close) are selected.
fn generate_random_mask(num_squares: u64, r: &[u8]) -> [bool; 25] {
    let mut new_mask = [false; 25];
    let mut selected = 0;
    for i in 0..25 {
        let rand_byte = r[i];
        let remaining_needed = num_squares as u64 - selected as u64;
        let remaining_positions = 25 - i;
        if remaining_needed > 0
            && (rand_byte as u64) * (remaining_positions as u64) < (remaining_needed * 256)
        {
            new_mask[i] = true;
            selected += 1;
        }
    }
    new_mask
}


