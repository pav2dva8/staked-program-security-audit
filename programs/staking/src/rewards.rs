use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::{
    constants::*, errors::PobError, guards::require_protocol_fee_vault, state::*,
    token::transfer_spl_tokens,
};

pub(crate) fn lock_multiplier_bps(lock_days: u16) -> Result<u64> {
    match lock_days {
        7 => Ok(10_000),
        30 => Ok(12_500),
        90 => Ok(17_500),
        180 => Ok(25_000),
        _ => err!(PobError::InvalidLockDuration),
    }
}

pub(crate) fn lock_unlock_ts(lock_days: u16) -> Result<i64> {
    Clock::get()?
        .unix_timestamp
        .checked_add(
            i64::from(lock_days)
                .checked_mul(DAY_SECONDS)
                .ok_or(PobError::MathOverflow)?,
        )
        .ok_or(PobError::MathOverflow.into())
}

pub(crate) fn lock_extension_allowed(current_unlock_ts: i64, next_unlock_ts: i64) -> Result<()> {
    require!(
        next_unlock_ts >= current_unlock_ts,
        PobError::CannotShortenLock
    );
    Ok(())
}

pub(crate) fn weighted_amount(amount: u64, multiplier_bps: u64) -> Result<u128> {
    (amount as u128)
        .checked_mul(multiplier_bps as u128)
        .ok_or(PobError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(PobError::MathOverflow.into())
}

pub(crate) fn apply_rewards(launch: &mut LaunchConfig, amount: u64) -> Result<()> {
    require!(launch.total_weighted_stake > 0, PobError::NoActiveStake);

    let increment = (amount as u128)
        .checked_mul(ACC_REWARD_PRECISION)
        .ok_or(PobError::MathOverflow)?
        .checked_div(launch.total_weighted_stake)
        .ok_or(PobError::MathOverflow)?;
    require!(increment > 0, PobError::NoRewards);

    launch.acc_reward_per_weight = launch
        .acc_reward_per_weight
        .checked_add(increment)
        .ok_or(PobError::MathOverflow)?;
    launch.reward_reserve = launch
        .reward_reserve
        .checked_add(amount)
        .ok_or(PobError::MathOverflow)?;

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RewardSplitAmounts {
    pub protocol_fee: u64,
    pub main_reward_amount: u64,
    pub reward_amount: u64,
}

pub(crate) fn reward_split_amounts(amount: u64) -> Result<RewardSplitAmounts> {
    require!(amount > 0, PobError::NoRewards);

    let protocol_fee = protocol_fee_amount(amount)?;
    let post_protocol = amount
        .checked_sub(protocol_fee)
        .ok_or(PobError::MathOverflow)?;
    let main_reward_amount = (post_protocol as u128)
        .checked_mul(MAIN_REWARD_BPS as u128)
        .ok_or(PobError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(PobError::MathOverflow)?;
    let main_reward_amount =
        u64::try_from(main_reward_amount).map_err(|_| PobError::MathOverflow)?;
    let reward_amount = post_protocol
        .checked_sub(main_reward_amount)
        .ok_or(PobError::MathOverflow)?;

    Ok(RewardSplitAmounts {
        protocol_fee,
        main_reward_amount,
        reward_amount,
    })
}

pub(crate) fn apply_reward_split_to_state(
    launch: &mut LaunchConfig,
    split: RewardSplitAmounts,
) -> Result<()> {
    if split.reward_amount > 0 {
        apply_rewards(launch, split.reward_amount)?;
    }

    Ok(())
}

pub(crate) fn main_reward_remaining_accounts<'info>(
    launch: &LaunchConfig,
    remaining_accounts: &'info [AccountInfo<'info>],
) -> Result<(
    Option<&'info AccountInfo<'info>>,
    &'info [AccountInfo<'info>],
)> {
    if launch.mint == STAKE_MAIN_MINT_ID {
        return Ok((None, remaining_accounts));
    }

    let (main_launch, rest) = remaining_accounts
        .split_first()
        .ok_or(PobError::InvalidMainRewardPool)?;
    Ok((Some(main_launch), rest))
}

pub(crate) fn apply_rewards_with_protocol_fee_and_main_reward<'info>(
    launch: &mut Account<'info, LaunchConfig>,
    main_launch_info: Option<&'info AccountInfo<'info>>,
    protocol_fee_vault: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    protocol_fee_vault_bump: u8,
    amount: u64,
) -> Result<()> {
    let split = reward_split_amounts(amount)?;

    if split.protocol_fee > 0 {
        ensure_protocol_fee_vault(
            payer,
            protocol_fee_vault,
            system_program_info,
            &launch.mint,
            protocol_fee_vault_bump,
        )?;
        move_lamports(
            &launch.to_account_info(),
            protocol_fee_vault,
            split.protocol_fee,
        )?;
    }

    if launch.mint == STAKE_MAIN_MINT_ID {
        let reward_amount = split
            .reward_amount
            .checked_add(split.main_reward_amount)
            .ok_or(PobError::MathOverflow)?;
        if reward_amount > 0 {
            apply_rewards(launch, reward_amount)?;
        }
        return Ok(());
    }

    apply_reward_split_to_state(launch, split)?;

    if split.main_reward_amount > 0 {
        let main_launch_info = main_launch_info.ok_or(PobError::InvalidMainRewardPool)?;
        require_main_reward_pool(main_launch_info)?;
        move_lamports(
            &launch.to_account_info(),
            main_launch_info,
            split.main_reward_amount,
        )?;

        let mut main_launch = Account::<LaunchConfig>::try_from(main_launch_info)?;
        apply_rewards(&mut main_launch, split.main_reward_amount)?;
        main_launch.exit(&crate::ID)?;
    }

    Ok(())
}

fn require_main_reward_pool<'info>(main_launch_info: &'info AccountInfo<'info>) -> Result<()> {
    let expected_launch =
        Pubkey::find_program_address(&[b"launch", STAKE_MAIN_MINT_ID.as_ref()], &crate::ID).0;

    require_keys_eq!(
        main_launch_info.key(),
        expected_launch,
        PobError::InvalidMainRewardPool
    );
    require!(
        main_launch_info.is_writable,
        PobError::InvalidMainRewardPool
    );
    require_keys_eq!(
        *main_launch_info.owner,
        crate::ID,
        PobError::InvalidMainRewardPool
    );

    let main_launch = Account::<LaunchConfig>::try_from(main_launch_info)?;
    require_keys_eq!(
        main_launch.mint,
        STAKE_MAIN_MINT_ID,
        PobError::InvalidMainRewardPool
    );

    Ok(())
}

pub(crate) fn protocol_fee_amount(amount: u64) -> Result<u64> {
    amount
        .checked_mul(PROTOCOL_FEE_BPS)
        .ok_or(PobError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(PobError::MathOverflow.into())
}

pub(crate) fn pending_reward(position: &StakePosition, launch: &LaunchConfig) -> Result<u64> {
    let accrued = reward_debt(position.weight, launch.acc_reward_per_weight)?;
    let pending = accrued
        .checked_sub(position.reward_debt)
        .ok_or(PobError::MathOverflow)?;

    u64::try_from(pending).map_err(|_| PobError::MathOverflow.into())
}

pub(crate) fn reward_debt(weight: u128, acc_reward_per_weight: u128) -> Result<u128> {
    weight
        .checked_mul(acc_reward_per_weight)
        .ok_or(PobError::MathOverflow)?
        .checked_div(ACC_REWARD_PRECISION)
        .ok_or(PobError::MathOverflow.into())
}

pub(crate) fn pay_rewards<'info>(
    launch: &mut Account<'info, LaunchConfig>,
    receiver: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    require!(
        launch.reward_reserve >= amount,
        PobError::InsufficientRewardReserve
    );

    launch.reward_reserve = launch
        .reward_reserve
        .checked_sub(amount)
        .ok_or(PobError::MathOverflow)?;

    move_lamports(&launch.to_account_info(), receiver, amount)
}

pub(crate) fn apply_quote_rewards(
    reward_pool: &mut QuoteRewardPool,
    total_weighted_stake: u128,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, PobError::NoRewards);
    require!(total_weighted_stake > 0, PobError::NoActiveStake);

    let increment = (amount as u128)
        .checked_mul(ACC_REWARD_PRECISION)
        .ok_or(PobError::MathOverflow)?
        .checked_div(total_weighted_stake)
        .ok_or(PobError::MathOverflow)?;
    require!(increment > 0, PobError::NoRewards);

    reward_pool.acc_reward_per_weight = reward_pool
        .acc_reward_per_weight
        .checked_add(increment)
        .ok_or(PobError::MathOverflow)?;
    reward_pool.reward_reserve = reward_pool
        .reward_reserve
        .checked_add(amount)
        .ok_or(PobError::MathOverflow)?;
    reward_pool.last_reward_ts = Clock::get()?.unix_timestamp;

    Ok(())
}

pub(crate) fn pending_quote_reward(
    quote_stake: &QuoteStakeState,
    reward_pool: &QuoteRewardPool,
) -> Result<u64> {
    let accrued = reward_debt(
        quote_stake.recorded_weight,
        reward_pool.acc_reward_per_weight,
    )?;
    let pending = accrued
        .checked_sub(quote_stake.reward_debt)
        .ok_or(PobError::MathOverflow)?;

    u64::try_from(pending).map_err(|_| PobError::MathOverflow.into())
}

pub(crate) fn pay_quote_rewards<'info>(
    reward_pool: &mut Account<'info, QuoteRewardPool>,
    reward_vault: &AccountInfo<'info>,
    receiver_ata: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
    reward_pool_bump: u8,
) -> Result<()> {
    require!(
        reward_pool.reward_reserve >= amount,
        PobError::InsufficientRewardReserve
    );

    reward_pool.reward_reserve = reward_pool
        .reward_reserve
        .checked_sub(amount)
        .ok_or(PobError::MathOverflow)?;

    let launch = reward_pool.launch;
    let quote_mint = reward_pool.quote_mint;
    let signer_seeds: &[&[&[u8]]] = &[&[
        b"quote-rewards",
        launch.as_ref(),
        quote_mint.as_ref(),
        &[reward_pool_bump],
    ]];
    transfer_spl_tokens(
        token_program,
        reward_vault,
        receiver_ata,
        &reward_pool.to_account_info(),
        amount,
        &reward_pool.quote_token_program,
        signer_seeds,
    )
}

pub(crate) fn apply_quote_rewards_with_protocol_fee<'info>(
    reward_pool: &mut Account<'info, QuoteRewardPool>,
    fee_owner_token_ata: &AccountInfo<'info>,
    quote_reward_vault_ata: &AccountInfo<'info>,
    quote_protocol_fee_vault_ata: &AccountInfo<'info>,
    quote_token_program: &AccountInfo<'info>,
    fee_owner: &AccountInfo<'info>,
    launch_mint: &Pubkey,
    total_weighted_stake: u128,
    fee_owner_bump: u8,
    amount: u64,
) -> Result<()> {
    let protocol_fee = protocol_fee_amount(amount)?;
    let reward_amount = amount
        .checked_sub(protocol_fee)
        .ok_or(PobError::MathOverflow)?;

    let signer_seeds: &[&[&[u8]]] = &[&[b"fee-owner", launch_mint.as_ref(), &[fee_owner_bump]]];

    if reward_amount > 0 {
        transfer_spl_tokens(
            quote_token_program,
            fee_owner_token_ata,
            quote_reward_vault_ata,
            fee_owner,
            reward_amount,
            &reward_pool.quote_token_program,
            signer_seeds,
        )?;
    }

    if protocol_fee > 0 {
        transfer_spl_tokens(
            quote_token_program,
            fee_owner_token_ata,
            quote_protocol_fee_vault_ata,
            fee_owner,
            protocol_fee,
            &reward_pool.quote_token_program,
            signer_seeds,
        )?;
        reward_pool.protocol_fee_reserve = reward_pool
            .protocol_fee_reserve
            .checked_add(protocol_fee)
            .ok_or(PobError::MathOverflow)?;
    }

    apply_quote_rewards(reward_pool, total_weighted_stake, reward_amount)
}

pub(crate) fn sweep_fee_owner_to_launch<'info>(
    fee_owner: &AccountInfo<'info>,
    launch: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    mint: &Pubkey,
    fee_owner_bump: u8,
    rent_floor: u64,
) -> Result<u64> {
    let amount = fee_owner
        .lamports()
        .checked_sub(rent_floor)
        .ok_or(PobError::MathOverflow)?;
    if amount == 0 {
        return Ok(0);
    }

    system_program::transfer(
        CpiContext::new_with_signer(
            system_program_info.clone(),
            system_program::Transfer {
                from: fee_owner.clone(),
                to: launch.clone(),
            },
            &[&[b"fee-owner", mint.as_ref(), &[fee_owner_bump]]],
        ),
        amount,
    )?;

    Ok(amount)
}

pub(crate) fn ensure_fee_owner_rent_exempt<'info>(
    payer: &AccountInfo<'info>,
    fee_owner: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
) -> Result<u64> {
    let rent_floor = Rent::get()?.minimum_balance(0);
    let current_lamports = fee_owner.lamports();
    if current_lamports >= rent_floor {
        return Ok(rent_floor);
    }

    let shortfall = rent_floor
        .checked_sub(current_lamports)
        .ok_or(PobError::MathOverflow)?;

    system_program::transfer(
        CpiContext::new(
            system_program_info.clone(),
            system_program::Transfer {
                from: payer.clone(),
                to: fee_owner.clone(),
            },
        ),
        shortfall,
    )?;

    Ok(rent_floor)
}

pub(crate) fn ensure_protocol_fee_vault<'info>(
    payer: &AccountInfo<'info>,
    protocol_fee_vault: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    mint: &Pubkey,
    protocol_fee_vault_bump: u8,
) -> Result<()> {
    require_protocol_fee_vault(protocol_fee_vault, mint)?;

    let rent_floor = Rent::get()?.minimum_balance(0);
    let current_lamports = protocol_fee_vault.lamports();
    if current_lamports >= rent_floor {
        return Ok(());
    }

    if current_lamports > 0 {
        let shortfall = rent_floor
            .checked_sub(current_lamports)
            .ok_or(PobError::MathOverflow)?;
        system_program::transfer(
            CpiContext::new(
                system_program_info.clone(),
                system_program::Transfer {
                    from: payer.clone(),
                    to: protocol_fee_vault.clone(),
                },
            ),
            shortfall,
        )?;

        return Ok(());
    }

    system_program::create_account(
        CpiContext::new_with_signer(
            system_program_info.clone(),
            system_program::CreateAccount {
                from: payer.clone(),
                to: protocol_fee_vault.clone(),
            },
            &[&[b"protocol-fees", mint.as_ref(), &[protocol_fee_vault_bump]]],
        ),
        rent_floor,
        0,
        &system_program::ID,
    )?;

    Ok(())
}

pub(crate) fn protocol_fee_vault_claimable(protocol_fee_vault: &AccountInfo) -> Result<u64> {
    let rent_floor = Rent::get()?.minimum_balance(0);
    if protocol_fee_vault.lamports() <= rent_floor {
        return Ok(0);
    }

    protocol_fee_vault
        .lamports()
        .checked_sub(rent_floor)
        .ok_or(PobError::MathOverflow.into())
}

pub(crate) fn move_lamports<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    **from.try_borrow_mut_lamports()? = from
        .lamports()
        .checked_sub(amount)
        .ok_or(PobError::MathOverflow)?;
    **to.try_borrow_mut_lamports()? = to
        .lamports()
        .checked_add(amount)
        .ok_or(PobError::MathOverflow)?;

    Ok(())
}
