use anchor_lang::prelude::*;

use crate::{
    account_contexts::*,
    constants::*,
    errors::PobError,
    guards::*,
    rewards::*,
    state::*,
    token::{require_associated_token_account, require_mint_account},
};
pub(crate) fn claim_rewards<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimRewards<'info>>,
) -> Result<()> {
    if !ctx.remaining_accounts.is_empty() {
        return claim_quote_rewards_from_remaining(ctx);
    }

    let pending = pending_reward(&ctx.accounts.stake_position, &ctx.accounts.launch)?;
    require!(pending > 0, PobError::NoRewards);

    let launch = &mut ctx.accounts.launch;
    let position = &mut ctx.accounts.stake_position;
    position.reward_debt = reward_debt(position.weight, launch.acc_reward_per_weight)?;
    pay_rewards(launch, &ctx.accounts.owner.to_account_info(), pending)?;

    Ok(())
}

pub(crate) fn initialize_quote_rewards(ctx: Context<InitializeQuoteRewards>) -> Result<()> {
    let quote_token_program = require_mint_account(&ctx.accounts.quote_mint.to_account_info())?;
    require_keys_eq!(
        ctx.accounts.quote_token_program.key(),
        quote_token_program,
        PobError::InvalidTokenAccount
    );
    require_keys_neq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );

    let reward_pool = &mut ctx.accounts.quote_reward_pool;
    reward_pool.launch = ctx.accounts.launch.key();
    reward_pool.quote_mint = ctx.accounts.quote_mint.key();
    reward_pool.quote_token_program = quote_token_program;
    reward_pool.acc_reward_per_weight = 0;
    reward_pool.reward_reserve = 0;
    reward_pool.protocol_fee_reserve = 0;
    reward_pool.last_reward_ts = 0;

    Ok(())
}

pub(crate) fn initialize_quote_stake(ctx: Context<InitializeQuoteStake>) -> Result<()> {
    require_quote_reward_pool(
        &ctx.accounts.quote_reward_pool,
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_reward_pool.quote_mint,
        &ctx.accounts.quote_reward_pool.quote_token_program,
    )?;

    let quote_stake = &mut ctx.accounts.quote_stake;
    quote_stake.stake_position = ctx.accounts.stake_position.key();
    quote_stake.reward_pool = ctx.accounts.quote_reward_pool.key();
    quote_stake.recorded_weight = ctx.accounts.stake_position.weight;
    quote_stake.reward_debt = reward_debt(
        ctx.accounts.stake_position.weight,
        ctx.accounts.quote_reward_pool.acc_reward_per_weight,
    )?;

    Ok(())
}

pub(crate) fn claim_quote_rewards_from_remaining<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimRewards<'info>>,
) -> Result<()> {
    let remaining = ctx.remaining_accounts;
    require!(remaining.len() == 6, PobError::InvalidTokenAccount);

    let quote_reward_pool_info = &remaining[0];
    let quote_mint = &remaining[1];
    let quote_token_program = &remaining[2];
    let quote_reward_vault_ata = &remaining[3];
    let user_quote_ata = &remaining[4];
    let quote_stake_info = &remaining[5];

    require!(
        quote_reward_pool_info.is_writable,
        PobError::InvalidTokenAccount
    );
    require!(
        quote_reward_vault_ata.is_writable,
        PobError::InvalidTokenAccount
    );
    require!(user_quote_ata.is_writable, PobError::InvalidTokenAccount);
    require!(quote_stake_info.is_writable, PobError::InvalidTokenAccount);
    require_keys_neq!(
        quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );

    let launch_key = ctx.accounts.launch.key();
    let quote_mint_key = quote_mint.key();
    let (expected_reward_pool, reward_pool_bump) = Pubkey::find_program_address(
        &[
            b"quote-rewards",
            launch_key.as_ref(),
            quote_mint_key.as_ref(),
        ],
        ctx.program_id,
    );
    require_keys_eq!(
        quote_reward_pool_info.key(),
        expected_reward_pool,
        PobError::InvalidTokenAccount
    );

    let mut quote_reward_pool: Account<QuoteRewardPool> =
        Account::try_from(quote_reward_pool_info)?;
    require_quote_reward_pool(
        &quote_reward_pool,
        &launch_key,
        &quote_mint_key,
        &quote_token_program.key(),
    )?;
    require_associated_token_account(
        quote_reward_vault_ata,
        &quote_reward_pool.key(),
        &quote_mint_key,
        &quote_token_program.key(),
    )?;
    require_associated_token_account(
        user_quote_ata,
        &ctx.accounts.owner.key(),
        &quote_mint_key,
        &quote_token_program.key(),
    )?;

    let stake_position_key = ctx.accounts.stake_position.key();
    let expected_quote_stake = Pubkey::find_program_address(
        &[
            b"quote-stake",
            stake_position_key.as_ref(),
            quote_reward_pool_info.key().as_ref(),
        ],
        ctx.program_id,
    )
    .0;
    require_keys_eq!(
        quote_stake_info.key(),
        expected_quote_stake,
        PobError::InvalidTokenAccount
    );

    let mut quote_stake: Account<QuoteStakeState> = Account::try_from(quote_stake_info)?;
    require_quote_stake(&quote_stake, &stake_position_key, &quote_reward_pool.key())?;

    let pending = pending_quote_reward(&quote_stake, &quote_reward_pool)?;
    if pending > 0 {
        pay_quote_rewards(
            &mut quote_reward_pool,
            quote_reward_vault_ata,
            user_quote_ata,
            quote_token_program,
            pending,
            reward_pool_bump,
        )?;
    }

    quote_stake.recorded_weight = ctx.accounts.stake_position.weight;
    quote_stake.reward_debt = reward_debt(
        ctx.accounts.stake_position.weight,
        quote_reward_pool.acc_reward_per_weight,
    )?;
    quote_reward_pool.exit(ctx.program_id)?;
    quote_stake.exit(ctx.program_id)
}
