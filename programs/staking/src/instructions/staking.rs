use anchor_lang::prelude::*;

use crate::{
    account_contexts::*,
    errors::PobError,
    guards::*,
    rewards::*,
    token::{require_mint_account, require_token_account, transfer_spl_tokens},
};
pub(crate) fn initialize_launch(ctx: Context<InitializeLaunch>) -> Result<()> {
    let token_program = require_mint_account(&ctx.accounts.mint.to_account_info())?;
    require_canonical_staking_vault(
        &ctx.accounts.staking_vault.key(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.mint.key(),
        &token_program,
    )?;
    require_token_account(
        &ctx.accounts.staking_vault.to_account_info(),
        &ctx.accounts.mint.key(),
        &ctx.accounts.launch.key(),
        &token_program,
    )?;
    let fee_owner = ctx.accounts.fee_owner.key();
    require_verified_bonding_curve(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.mint.key(),
        &fee_owner,
    )?;

    let launch = &mut ctx.accounts.launch;
    launch.mint = ctx.accounts.mint.key();
    launch.token_program = token_program;
    launch.total_weighted_stake = 0;
    launch.acc_reward_per_weight = 0;
    launch.reward_reserve = 0;

    Ok(())
}

pub(crate) fn stake(ctx: Context<Stake>, amount: u64, lock_days: u16) -> Result<()> {
    require!(amount > 0, PobError::ZeroAmount);
    require_canonical_staking_vault(
        &ctx.accounts.staking_vault.key(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.launch.token_program,
    )?;
    require_token_account(
        &ctx.accounts.user_token.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.staker.key(),
        &ctx.accounts.launch.token_program,
    )?;
    require_token_account(
        &ctx.accounts.staking_vault.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.launch.key(),
        &ctx.accounts.launch.token_program,
    )?;
    if ctx.accounts.launch.total_weighted_stake > 0 {
        require_clean_creator_vault(
            &ctx.accounts.creator_vault.to_account_info(),
            &ctx.accounts.launch.mint,
        )?;
        require_clean_pumpswap_creator_vault(
            &ctx.accounts.coin_creator_vault_ata.to_account_info(),
            &ctx.accounts.launch.mint,
        )?;
    }

    let multiplier_bps = lock_multiplier_bps(lock_days)?;
    let weight = weighted_amount(amount, multiplier_bps)?;
    let unlock_ts = lock_unlock_ts(lock_days)?;

    transfer_spl_tokens(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.user_token.to_account_info(),
        &ctx.accounts.staking_vault.to_account_info(),
        &ctx.accounts.staker.to_account_info(),
        amount,
        &ctx.accounts.launch.token_program,
        &[],
    )?;

    let launch = &mut ctx.accounts.launch;
    let position = &mut ctx.accounts.stake_position;
    position.amount = amount;
    position.weight = weight;
    position.unlock_ts = unlock_ts;
    position.reward_debt = reward_debt(weight, launch.acc_reward_per_weight)?;

    launch.total_weighted_stake = launch
        .total_weighted_stake
        .checked_add(weight)
        .ok_or(PobError::MathOverflow)?;

    Ok(())
}

pub(crate) fn increase_stake(
    ctx: Context<IncreaseStake>,
    amount: u64,
    lock_days: u16,
) -> Result<()> {
    require!(amount > 0, PobError::ZeroAmount);
    require_canonical_staking_vault(
        &ctx.accounts.staking_vault.key(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.launch.token_program,
    )?;
    require_token_account(
        &ctx.accounts.user_token.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.staker.key(),
        &ctx.accounts.launch.token_program,
    )?;
    require_token_account(
        &ctx.accounts.staking_vault.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.launch.key(),
        &ctx.accounts.launch.token_program,
    )?;
    require_clean_creator_vault(
        &ctx.accounts.creator_vault.to_account_info(),
        &ctx.accounts.launch.mint,
    )?;
    require_clean_pumpswap_creator_vault(
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.launch.mint,
    )?;

    let multiplier_bps = lock_multiplier_bps(lock_days)?;
    let unlock_ts = lock_unlock_ts(lock_days)?;
    let pending = pending_reward(&ctx.accounts.stake_position, &ctx.accounts.launch)?;
    lock_extension_allowed(ctx.accounts.stake_position.unlock_ts, unlock_ts)?;

    transfer_spl_tokens(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.user_token.to_account_info(),
        &ctx.accounts.staking_vault.to_account_info(),
        &ctx.accounts.staker.to_account_info(),
        amount,
        &ctx.accounts.launch.token_program,
        &[],
    )?;

    let launch = &mut ctx.accounts.launch;
    if pending > 0 {
        pay_rewards(launch, &ctx.accounts.staker.to_account_info(), pending)?;
    }

    let position = &mut ctx.accounts.stake_position;
    let new_amount = position
        .amount
        .checked_add(amount)
        .ok_or(PobError::MathOverflow)?;
    let new_weight = weighted_amount(new_amount, multiplier_bps)?;

    launch.total_weighted_stake = launch
        .total_weighted_stake
        .checked_sub(position.weight)
        .ok_or(PobError::MathOverflow)?
        .checked_add(new_weight)
        .ok_or(PobError::MathOverflow)?;

    position.amount = new_amount;
    position.weight = new_weight;
    position.unlock_ts = unlock_ts;
    position.reward_debt = reward_debt(new_weight, launch.acc_reward_per_weight)?;

    Ok(())
}

pub(crate) fn unstake(ctx: Context<Unstake>) -> Result<()> {
    require!(
        Clock::get()?.unix_timestamp >= ctx.accounts.stake_position.unlock_ts,
        PobError::PositionLocked
    );
    require_canonical_staking_vault(
        &ctx.accounts.staking_vault.key(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.launch.token_program,
    )?;
    require_token_account(
        &ctx.accounts.staking_vault.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.launch.key(),
        &ctx.accounts.launch.token_program,
    )?;
    require_token_account(
        &ctx.accounts.user_token.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.owner.key(),
        &ctx.accounts.launch.token_program,
    )?;

    let pending = pending_reward(&ctx.accounts.stake_position, &ctx.accounts.launch)?;
    let launch = &mut ctx.accounts.launch;
    let position = &ctx.accounts.stake_position;

    if pending > 0 {
        pay_rewards(launch, &ctx.accounts.owner.to_account_info(), pending)?;
    }

    launch.total_weighted_stake = launch
        .total_weighted_stake
        .checked_sub(position.weight)
        .ok_or(PobError::MathOverflow)?;

    let signer_seeds: &[&[&[u8]]] = &[&[b"launch", launch.mint.as_ref(), &[ctx.bumps.launch]]];

    transfer_spl_tokens(
        &ctx.accounts.token_program.to_account_info(),
        &ctx.accounts.staking_vault.to_account_info(),
        &ctx.accounts.user_token.to_account_info(),
        &launch.to_account_info(),
        position.amount,
        &launch.token_program,
        signer_seeds,
    )?;

    Ok(())
}
