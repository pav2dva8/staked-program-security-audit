use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use crate::{
    account_contexts::*, constants::*, errors::PobError, guards::*, pda::*, rewards::*, token::*,
};

pub(crate) fn claim_pump_creator_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimPumpCreatorFees<'info>>,
) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_verified_bonding_curve(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.fee_owner.key(),
    )?;
    require_keys_eq!(
        ctx.accounts.creator_vault.key(),
        pump_creator_vault_pda(&ctx.accounts.fee_owner.key()).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_event_authority.key(),
        pump_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_program.key(),
        PUMP_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_protocol_fee_vault(
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.launch.mint,
    )?;

    let fee_owner_rent_floor = ensure_fee_owner_rent_exempt(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;

    let ix = Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(ctx.accounts.fee_owner.key(), true),
            AccountMeta::new(ctx.accounts.creator_vault.key(), false),
            AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_event_authority.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_program.key(), false),
        ],
        data: PUMP_COLLECT_CREATOR_FEE_IX.to_vec(),
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.fee_owner.to_account_info(),
            ctx.accounts.creator_vault.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.pump_event_authority.to_account_info(),
            ctx.accounts.pump_program.to_account_info(),
        ],
        &[&[
            b"fee-owner",
            ctx.accounts.launch.mint.as_ref(),
            &[ctx.bumps.fee_owner],
        ]],
    )?;

    let amount = sweep_fee_owner_to_launch(
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.launch.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.bumps.fee_owner,
        fee_owner_rent_floor,
    )?;
    require!(amount > 0, PobError::NoRewards);

    let (main_launch_info, _) =
        main_reward_remaining_accounts(&ctx.accounts.launch, ctx.remaining_accounts)?;
    apply_rewards_with_protocol_fee_and_main_reward(
        &mut ctx.accounts.launch,
        main_launch_info,
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.protocol_fee_vault,
        amount,
    )?;

    Ok(())
}

pub(crate) fn claim_pump_quote_creator_fees(ctx: Context<ClaimPumpQuoteCreatorFees>) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_keys_neq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );
    require_verified_bonding_curve(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.fee_owner.key(),
    )?;
    require_keys_eq!(
        ctx.accounts.creator_vault.key(),
        pump_creator_vault_pda(&ctx.accounts.fee_owner.key()).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.associated_token_program.key(),
        ASSOCIATED_TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_event_authority.key(),
        pump_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_program.key(),
        PUMP_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_quote_reward_pool(
        &ctx.accounts.quote_reward_pool,
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_associated_token_account(
        &ctx.accounts.creator_vault_token_ata.to_account_info(),
        &ctx.accounts.creator_vault.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_quote_fee_accounts(
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.quote_reward_pool.key(),
        &ctx.accounts.quote_protocol_fee_authority.key(),
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;

    let before_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    let ix = Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(ctx.accounts.fee_owner.key(), true),
            AccountMeta::new(ctx.accounts.fee_owner_token_ata.key(), false),
            AccountMeta::new(ctx.accounts.creator_vault.key(), false),
            AccountMeta::new(ctx.accounts.creator_vault_token_ata.key(), false),
            AccountMeta::new_readonly(ctx.accounts.quote_mint.key(), false),
            AccountMeta::new_readonly(ctx.accounts.quote_token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.associated_token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_event_authority.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_program.key(), false),
        ],
        data: PUMP_COLLECT_CREATOR_FEE_V2_IX.to_vec(),
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.fee_owner.to_account_info(),
            ctx.accounts.fee_owner_token_ata.to_account_info(),
            ctx.accounts.creator_vault.to_account_info(),
            ctx.accounts.creator_vault_token_ata.to_account_info(),
            ctx.accounts.quote_mint.to_account_info(),
            ctx.accounts.quote_token_program.to_account_info(),
            ctx.accounts.associated_token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.pump_event_authority.to_account_info(),
            ctx.accounts.pump_program.to_account_info(),
        ],
        &[&[
            b"fee-owner",
            ctx.accounts.launch.mint.as_ref(),
            &[ctx.bumps.fee_owner],
        ]],
    )?;

    let after_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    let collected = after_amount
        .checked_sub(before_amount)
        .ok_or(PobError::MathOverflow)?;
    require!(collected > 0, PobError::NoRewards);

    apply_quote_rewards_with_protocol_fee(
        &mut ctx.accounts.quote_reward_pool,
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.accounts.launch.total_weighted_stake,
        ctx.bumps.fee_owner,
        collected,
    )?;

    Ok(())
}

pub(crate) fn claim_pump_shared_creator_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimPumpSharedCreatorFees<'info>>,
) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_keys_eq!(
        ctx.accounts.mint.key(),
        ctx.accounts.launch.mint,
        PobError::InvalidMint
    );
    require_keys_eq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.quote_token_program.key(),
        TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    require_keys_eq!(
        ctx.accounts.associated_token_program.key(),
        ASSOCIATED_TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    let shares = require_verified_shared_bonding_curve(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.sharing_config.to_account_info(),
    )?;
    require_keys_eq!(
        ctx.accounts.creator_vault.key(),
        pump_creator_vault_pda(&ctx.accounts.sharing_config.key()).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.creator_vault_quote_token_ata.key(),
        associated_token_address(
            &ctx.accounts.creator_vault.key(),
            &ctx.accounts.quote_token_program.key(),
            &ctx.accounts.quote_mint.key(),
        ),
        PobError::InvalidTokenAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_event_authority.key(),
        pump_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_program.key(),
        PUMP_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_protocol_fee_vault(
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.launch.mint,
    )?;
    let fee_owner_rent_floor = ensure_fee_owner_rent_exempt(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
    )?;
    let (main_launch_info, shareholder_accounts) =
        main_reward_remaining_accounts(&ctx.accounts.launch, ctx.remaining_accounts)?;

    distribute_pump_creator_fees_v2(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.sharing_config.to_account_info(),
        &ctx.accounts.creator_vault.to_account_info(),
        &ctx.accounts.creator_vault_quote_token_ata.to_account_info(),
        &ctx.accounts.quote_mint.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.associated_token_program.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.pump_event_authority.to_account_info(),
        &ctx.accounts.pump_program.to_account_info(),
        &shares,
        ShareholderAccountMode::System {
            fee_owner: ctx.accounts.fee_owner.to_account_info(),
        },
        shareholder_accounts,
    )?;

    let amount = sweep_fee_owner_to_launch(
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.launch.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.bumps.fee_owner,
        fee_owner_rent_floor,
    )?;
    require!(amount > 0, PobError::NoRewards);

    apply_rewards_with_protocol_fee_and_main_reward(
        &mut ctx.accounts.launch,
        main_launch_info,
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.protocol_fee_vault,
        amount,
    )?;

    Ok(())
}

pub(crate) fn claim_pump_shared_quote_creator_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimPumpSharedQuoteCreatorFees<'info>>,
) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_keys_eq!(
        ctx.accounts.mint.key(),
        ctx.accounts.launch.mint,
        PobError::InvalidMint
    );
    require_keys_neq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.associated_token_program.key(),
        ASSOCIATED_TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    let shares = require_verified_shared_bonding_curve(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.sharing_config.to_account_info(),
    )?;
    require_keys_eq!(
        ctx.accounts.creator_vault.key(),
        pump_creator_vault_pda(&ctx.accounts.sharing_config.key()).0,
        PobError::InvalidPumpAccount
    );
    require_quote_reward_pool(
        &ctx.accounts.quote_reward_pool,
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_associated_token_account(
        &ctx.accounts.creator_vault_quote_token_ata.to_account_info(),
        &ctx.accounts.creator_vault.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_quote_fee_accounts(
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.quote_reward_pool.key(),
        &ctx.accounts.quote_protocol_fee_authority.key(),
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_keys_eq!(
        ctx.accounts.pump_event_authority.key(),
        pump_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_program.key(),
        PUMP_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );

    let before_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    distribute_pump_creator_fees_v2(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.sharing_config.to_account_info(),
        &ctx.accounts.creator_vault.to_account_info(),
        &ctx.accounts.creator_vault_quote_token_ata.to_account_info(),
        &ctx.accounts.quote_mint.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.associated_token_program.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.pump_event_authority.to_account_info(),
        &ctx.accounts.pump_program.to_account_info(),
        &shares,
        ShareholderAccountMode::Token {
            fee_owner: ctx.accounts.fee_owner.to_account_info(),
            fee_owner_token_account: ctx.accounts.fee_owner_token_ata.to_account_info(),
            quote_mint: ctx.accounts.quote_mint.key(),
            quote_token_program: ctx.accounts.quote_token_program.key(),
        },
        ctx.remaining_accounts,
    )?;

    let after_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    let collected = after_amount
        .checked_sub(before_amount)
        .ok_or(PobError::MathOverflow)?;
    require!(collected > 0, PobError::NoRewards);

    apply_quote_rewards_with_protocol_fee(
        &mut ctx.accounts.quote_reward_pool,
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.accounts.launch.total_weighted_stake,
        ctx.bumps.fee_owner,
        collected,
    )?;

    Ok(())
}

pub(crate) fn claim_pumpswap_creator_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimPumpSwapCreatorFees<'info>>,
) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_keys_eq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.quote_token_program.key(),
        TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    require_keys_eq!(
        ctx.accounts.coin_creator_vault_authority.key(),
        pump_amm_creator_vault_authority_pda(&ctx.accounts.fee_owner.key()).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_event_authority.key(),
        pump_amm_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_program.key(),
        PUMP_AMM_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_associated_token_account(
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.coin_creator_vault_authority.key(),
        &ctx.accounts.quote_mint.key(),
        &TOKEN_PROGRAM_ID,
    )?;
    require_associated_token_account(
        &ctx.accounts.fee_owner_wsol_ata.to_account_info(),
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.quote_mint.key(),
        &TOKEN_PROGRAM_ID,
    )?;
    require_protocol_fee_vault(
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.launch.mint,
    )?;

    let ix = Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(ctx.accounts.quote_mint.key(), false),
            AccountMeta::new_readonly(ctx.accounts.quote_token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.fee_owner.key(), false),
            AccountMeta::new_readonly(ctx.accounts.coin_creator_vault_authority.key(), false),
            AccountMeta::new(ctx.accounts.coin_creator_vault_ata.key(), false),
            AccountMeta::new(ctx.accounts.fee_owner_wsol_ata.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_amm_event_authority.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_amm_program.key(), false),
        ],
        data: PUMP_AMM_COLLECT_COIN_CREATOR_FEE_IX.to_vec(),
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.quote_mint.to_account_info(),
            ctx.accounts.quote_token_program.to_account_info(),
            ctx.accounts.fee_owner.to_account_info(),
            ctx.accounts.coin_creator_vault_authority.to_account_info(),
            ctx.accounts.coin_creator_vault_ata.to_account_info(),
            ctx.accounts.fee_owner_wsol_ata.to_account_info(),
            ctx.accounts.pump_amm_event_authority.to_account_info(),
            ctx.accounts.pump_amm_program.to_account_info(),
        ],
        &[],
    )?;

    let collected_wsol = token_account_amount(&ctx.accounts.fee_owner_wsol_ata.to_account_info())?;
    require!(collected_wsol > 0, PobError::NoRewards);

    let reward_lamports = ctx.accounts.fee_owner_wsol_ata.to_account_info().lamports();
    close_spl_token_account(
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.fee_owner_wsol_ata.to_account_info(),
        &ctx.accounts.launch.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &[&[
            b"fee-owner",
            ctx.accounts.launch.mint.as_ref(),
            &[ctx.bumps.fee_owner],
        ]],
    )?;
    require!(reward_lamports > 0, PobError::NoRewards);

    let (main_launch_info, _) =
        main_reward_remaining_accounts(&ctx.accounts.launch, ctx.remaining_accounts)?;
    apply_rewards_with_protocol_fee_and_main_reward(
        &mut ctx.accounts.launch,
        main_launch_info,
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.protocol_fee_vault,
        reward_lamports,
    )?;

    Ok(())
}

pub(crate) fn claim_pumpswap_quote_creator_fees(
    ctx: Context<ClaimPumpSwapQuoteCreatorFees>,
) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_keys_neq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.coin_creator_vault_authority.key(),
        pump_amm_creator_vault_authority_pda(&ctx.accounts.fee_owner.key()).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_event_authority.key(),
        pump_amm_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_program.key(),
        PUMP_AMM_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_quote_reward_pool(
        &ctx.accounts.quote_reward_pool,
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_associated_token_account(
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.coin_creator_vault_authority.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_quote_fee_accounts(
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.quote_reward_pool.key(),
        &ctx.accounts.quote_protocol_fee_authority.key(),
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;

    let before_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    let ix = Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(ctx.accounts.quote_mint.key(), false),
            AccountMeta::new_readonly(ctx.accounts.quote_token_program.key(), false),
            AccountMeta::new_readonly(ctx.accounts.fee_owner.key(), false),
            AccountMeta::new_readonly(ctx.accounts.coin_creator_vault_authority.key(), false),
            AccountMeta::new(ctx.accounts.coin_creator_vault_ata.key(), false),
            AccountMeta::new(ctx.accounts.fee_owner_token_ata.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_amm_event_authority.key(), false),
            AccountMeta::new_readonly(ctx.accounts.pump_amm_program.key(), false),
        ],
        data: PUMP_AMM_COLLECT_COIN_CREATOR_FEE_IX.to_vec(),
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.quote_mint.to_account_info(),
            ctx.accounts.quote_token_program.to_account_info(),
            ctx.accounts.fee_owner.to_account_info(),
            ctx.accounts.coin_creator_vault_authority.to_account_info(),
            ctx.accounts.coin_creator_vault_ata.to_account_info(),
            ctx.accounts.fee_owner_token_ata.to_account_info(),
            ctx.accounts.pump_amm_event_authority.to_account_info(),
            ctx.accounts.pump_amm_program.to_account_info(),
        ],
        &[],
    )?;

    let after_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    let collected = after_amount
        .checked_sub(before_amount)
        .ok_or(PobError::MathOverflow)?;
    require!(collected > 0, PobError::NoRewards);

    apply_quote_rewards_with_protocol_fee(
        &mut ctx.accounts.quote_reward_pool,
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.accounts.launch.total_weighted_stake,
        ctx.bumps.fee_owner,
        collected,
    )?;

    Ok(())
}

pub(crate) fn claim_pumpswap_shared_creator_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimPumpSwapSharedCreatorFees<'info>>,
) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_keys_eq!(
        ctx.accounts.mint.key(),
        ctx.accounts.launch.mint,
        PobError::InvalidMint
    );
    require_keys_eq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.quote_token_program.key(),
        TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    require_keys_eq!(
        ctx.accounts.associated_token_program.key(),
        ASSOCIATED_TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    let shares = require_verified_shared_bonding_curve(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.sharing_config.to_account_info(),
    )?;
    require_shared_pumpswap_vaults(
        &ctx.accounts.sharing_config.key(),
        &ctx.accounts.pump_creator_vault.to_account_info(),
        &ctx.accounts.pump_creator_vault_ata.to_account_info(),
        &ctx.accounts.coin_creator_vault_authority.to_account_info(),
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_associated_token_account(
        &ctx.accounts.fee_owner_wsol_ata.to_account_info(),
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_keys_eq!(
        ctx.accounts.pump_event_authority.key(),
        pump_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_program.key(),
        PUMP_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_event_authority.key(),
        pump_amm_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_program.key(),
        PUMP_AMM_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_protocol_fee_vault(
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.launch.mint,
    )?;
    let (main_launch_info, shareholder_accounts) =
        main_reward_remaining_accounts(&ctx.accounts.launch, ctx.remaining_accounts)?;
    transfer_pumpswap_creator_fees_to_pump_v2(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.quote_mint.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.associated_token_program.to_account_info(),
        &ctx.accounts.sharing_config.to_account_info(),
        &ctx.accounts.coin_creator_vault_authority.to_account_info(),
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.pump_creator_vault.to_account_info(),
        &ctx.accounts.pump_creator_vault_ata.to_account_info(),
        &ctx.accounts.pump_amm_event_authority.to_account_info(),
        &ctx.accounts.pump_amm_program.to_account_info(),
    )?;
    distribute_pump_creator_fees_v2(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.sharing_config.to_account_info(),
        &ctx.accounts.pump_creator_vault.to_account_info(),
        &ctx.accounts.pump_creator_vault_ata.to_account_info(),
        &ctx.accounts.quote_mint.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.associated_token_program.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.pump_event_authority.to_account_info(),
        &ctx.accounts.pump_program.to_account_info(),
        &shares,
        ShareholderAccountMode::Token {
            fee_owner: ctx.accounts.fee_owner.to_account_info(),
            fee_owner_token_account: ctx.accounts.fee_owner_wsol_ata.to_account_info(),
            quote_mint: ctx.accounts.quote_mint.key(),
            quote_token_program: ctx.accounts.quote_token_program.key(),
        },
        shareholder_accounts,
    )?;

    let collected_wsol = token_account_amount(&ctx.accounts.fee_owner_wsol_ata.to_account_info())?;
    require!(collected_wsol > 0, PobError::NoRewards);

    let reward_lamports = ctx.accounts.fee_owner_wsol_ata.to_account_info().lamports();
    close_spl_token_account(
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.fee_owner_wsol_ata.to_account_info(),
        &ctx.accounts.launch.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &[&[
            b"fee-owner",
            ctx.accounts.launch.mint.as_ref(),
            &[ctx.bumps.fee_owner],
        ]],
    )?;
    require!(reward_lamports > 0, PobError::NoRewards);

    apply_rewards_with_protocol_fee_and_main_reward(
        &mut ctx.accounts.launch,
        main_launch_info,
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ctx.bumps.protocol_fee_vault,
        reward_lamports,
    )?;

    Ok(())
}

pub(crate) fn claim_pumpswap_shared_quote_creator_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClaimPumpSwapSharedQuoteCreatorFees<'info>>,
) -> Result<()> {
    require!(
        ctx.accounts.launch.total_weighted_stake > 0,
        PobError::NoActiveStake
    );
    require_keys_eq!(
        ctx.accounts.mint.key(),
        ctx.accounts.launch.mint,
        PobError::InvalidMint
    );
    require_keys_neq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.associated_token_program.key(),
        ASSOCIATED_TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );
    let shares = require_verified_shared_bonding_curve(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.launch.mint,
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.sharing_config.to_account_info(),
    )?;
    require_quote_reward_pool(
        &ctx.accounts.quote_reward_pool,
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_shared_pumpswap_vaults(
        &ctx.accounts.sharing_config.key(),
        &ctx.accounts.pump_creator_vault.to_account_info(),
        &ctx.accounts.pump_creator_vault_ata.to_account_info(),
        &ctx.accounts.coin_creator_vault_authority.to_account_info(),
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_quote_fee_accounts(
        &ctx.accounts.fee_owner.key(),
        &ctx.accounts.quote_reward_pool.key(),
        &ctx.accounts.quote_protocol_fee_authority.key(),
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_keys_eq!(
        ctx.accounts.pump_event_authority.key(),
        pump_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_program.key(),
        PUMP_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_event_authority.key(),
        pump_amm_event_authority_pda().0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        ctx.accounts.pump_amm_program.key(),
        PUMP_AMM_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );

    let before_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    transfer_pumpswap_creator_fees_to_pump_v2(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.quote_mint.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.associated_token_program.to_account_info(),
        &ctx.accounts.sharing_config.to_account_info(),
        &ctx.accounts.coin_creator_vault_authority.to_account_info(),
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.pump_creator_vault.to_account_info(),
        &ctx.accounts.pump_creator_vault_ata.to_account_info(),
        &ctx.accounts.pump_amm_event_authority.to_account_info(),
        &ctx.accounts.pump_amm_program.to_account_info(),
    )?;
    distribute_pump_creator_fees_v2(
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.sharing_config.to_account_info(),
        &ctx.accounts.pump_creator_vault.to_account_info(),
        &ctx.accounts.pump_creator_vault_ata.to_account_info(),
        &ctx.accounts.quote_mint.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.associated_token_program.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        &ctx.accounts.pump_event_authority.to_account_info(),
        &ctx.accounts.pump_program.to_account_info(),
        &shares,
        ShareholderAccountMode::Token {
            fee_owner: ctx.accounts.fee_owner.to_account_info(),
            fee_owner_token_account: ctx.accounts.fee_owner_token_ata.to_account_info(),
            quote_mint: ctx.accounts.quote_mint.key(),
            quote_token_program: ctx.accounts.quote_token_program.key(),
        },
        ctx.remaining_accounts,
    )?;

    let after_amount = token_account_amount(&ctx.accounts.fee_owner_token_ata.to_account_info())?;
    let collected = after_amount
        .checked_sub(before_amount)
        .ok_or(PobError::MathOverflow)?;
    require!(collected > 0, PobError::NoRewards);

    apply_quote_rewards_with_protocol_fee(
        &mut ctx.accounts.quote_reward_pool,
        &ctx.accounts.fee_owner_token_ata.to_account_info(),
        &ctx.accounts.quote_reward_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.fee_owner.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.accounts.launch.total_weighted_stake,
        ctx.bumps.fee_owner,
        collected,
    )?;

    Ok(())
}

enum ShareholderAccountMode<'info> {
    System {
        fee_owner: AccountInfo<'info>,
    },
    Token {
        fee_owner: AccountInfo<'info>,
        fee_owner_token_account: AccountInfo<'info>,
        quote_mint: Pubkey,
        quote_token_program: Pubkey,
    },
}

fn distribute_pump_creator_fees_v2<'info>(
    payer: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    bonding_curve: &AccountInfo<'info>,
    sharing_config: &AccountInfo<'info>,
    creator_vault: &AccountInfo<'info>,
    creator_vault_quote_token_account: &AccountInfo<'info>,
    quote_mint: &AccountInfo<'info>,
    quote_token_program: &AccountInfo<'info>,
    associated_token_program: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    pump_event_authority: &AccountInfo<'info>,
    pump_program: &AccountInfo<'info>,
    shares: &[PumpFeeShare],
    shareholder_mode: ShareholderAccountMode<'info>,
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let mut accounts = vec![
        AccountMeta::new(payer.key(), true),
        AccountMeta::new_readonly(mint.key(), false),
        AccountMeta::new_readonly(bonding_curve.key(), false),
        AccountMeta::new_readonly(sharing_config.key(), false),
        AccountMeta::new(creator_vault.key(), false),
        AccountMeta::new_readonly(system_program.key(), false),
        AccountMeta::new_readonly(pump_event_authority.key(), false),
        AccountMeta::new_readonly(pump_program.key(), false),
        AccountMeta::new(creator_vault_quote_token_account.key(), false),
        AccountMeta::new_readonly(quote_mint.key(), false),
        AccountMeta::new_readonly(quote_token_program.key(), false),
        AccountMeta::new_readonly(associated_token_program.key(), false),
    ];
    let mut infos = vec![
        payer.clone(),
        mint.clone(),
        bonding_curve.clone(),
        sharing_config.clone(),
        creator_vault.clone(),
        system_program.clone(),
        pump_event_authority.clone(),
        pump_program.clone(),
        creator_vault_quote_token_account.clone(),
        quote_mint.clone(),
        quote_token_program.clone(),
        associated_token_program.clone(),
    ];

    append_shareholder_accounts(
        &mut accounts,
        &mut infos,
        shares,
        shareholder_mode,
        remaining_accounts,
    )?;

    let mut data = PUMP_DISTRIBUTE_CREATOR_FEES_V2_IX.to_vec();
    data.push(0);
    let ix = Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts,
        data,
    };

    invoke_signed(&ix, &infos, &[])?;

    Ok(())
}

fn transfer_pumpswap_creator_fees_to_pump_v2<'info>(
    payer: &AccountInfo<'info>,
    quote_mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    associated_token_program: &AccountInfo<'info>,
    coin_creator: &AccountInfo<'info>,
    coin_creator_vault_authority: &AccountInfo<'info>,
    coin_creator_vault_ata: &AccountInfo<'info>,
    pump_creator_vault: &AccountInfo<'info>,
    pump_creator_vault_ata: &AccountInfo<'info>,
    pump_amm_event_authority: &AccountInfo<'info>,
    pump_amm_program: &AccountInfo<'info>,
) -> Result<()> {
    let ix = Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.key(), true),
            AccountMeta::new_readonly(quote_mint.key(), false),
            AccountMeta::new_readonly(token_program.key(), false),
            AccountMeta::new_readonly(system_program.key(), false),
            AccountMeta::new_readonly(associated_token_program.key(), false),
            AccountMeta::new_readonly(coin_creator.key(), false),
            AccountMeta::new(coin_creator_vault_authority.key(), false),
            AccountMeta::new(coin_creator_vault_ata.key(), false),
            AccountMeta::new(pump_creator_vault.key(), false),
            AccountMeta::new(pump_creator_vault_ata.key(), false),
            AccountMeta::new_readonly(pump_amm_event_authority.key(), false),
            AccountMeta::new_readonly(pump_amm_program.key(), false),
        ],
        data: PUMP_AMM_TRANSFER_CREATOR_FEES_TO_PUMP_V2_IX.to_vec(),
    };

    invoke_signed(
        &ix,
        &[
            payer.clone(),
            quote_mint.clone(),
            token_program.clone(),
            system_program.clone(),
            associated_token_program.clone(),
            coin_creator.clone(),
            coin_creator_vault_authority.clone(),
            coin_creator_vault_ata.clone(),
            pump_creator_vault.clone(),
            pump_creator_vault_ata.clone(),
            pump_amm_event_authority.clone(),
            pump_amm_program.clone(),
        ],
        &[],
    )?;

    Ok(())
}

fn append_shareholder_accounts<'info>(
    accounts: &mut Vec<AccountMeta>,
    infos: &mut Vec<AccountInfo<'info>>,
    shares: &[PumpFeeShare],
    mode: ShareholderAccountMode<'info>,
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let mut remaining_index = 0usize;

    match mode {
        ShareholderAccountMode::System { fee_owner } => {
            for share in shares {
                if share.address == fee_owner.key() {
                    accounts.push(AccountMeta::new(fee_owner.key(), false));
                    infos.push(fee_owner.clone());
                    continue;
                }

                let recipient = remaining_accounts
                    .get(remaining_index)
                    .ok_or(PobError::InvalidPumpAccount)?;
                remaining_index = remaining_index
                    .checked_add(1)
                    .ok_or(PobError::MathOverflow)?;
                require_keys_eq!(recipient.key(), share.address, PobError::InvalidPumpAccount);
                accounts.push(AccountMeta::new(recipient.key(), false));
                infos.push(recipient.clone());
            }
        }
        ShareholderAccountMode::Token {
            fee_owner,
            fee_owner_token_account,
            quote_mint,
            quote_token_program,
        } => {
            for share in shares {
                if share.address == fee_owner.key() {
                    accounts.push(AccountMeta::new(fee_owner.key(), false));
                    infos.push(fee_owner.clone());
                    continue;
                }

                let recipient = remaining_accounts
                    .get(remaining_index)
                    .ok_or(PobError::InvalidPumpAccount)?;
                remaining_index = remaining_index
                    .checked_add(1)
                    .ok_or(PobError::MathOverflow)?;
                require_keys_eq!(recipient.key(), share.address, PobError::InvalidPumpAccount);
                accounts.push(AccountMeta::new(recipient.key(), false));
                infos.push(recipient.clone());
            }

            for share in shares {
                let expected_token_account =
                    associated_token_address(&share.address, &quote_token_program, &quote_mint);
                if share.address == fee_owner.key() {
                    require_keys_eq!(
                        fee_owner_token_account.key(),
                        expected_token_account,
                        PobError::InvalidTokenAccount
                    );
                    accounts.push(AccountMeta::new(fee_owner_token_account.key(), false));
                    infos.push(fee_owner_token_account.clone());
                    continue;
                }

                let recipient_token_account = remaining_accounts
                    .get(remaining_index)
                    .ok_or(PobError::InvalidPumpAccount)?;
                remaining_index = remaining_index
                    .checked_add(1)
                    .ok_or(PobError::MathOverflow)?;
                require_keys_eq!(
                    recipient_token_account.key(),
                    expected_token_account,
                    PobError::InvalidTokenAccount
                );
                accounts.push(AccountMeta::new(recipient_token_account.key(), false));
                infos.push(recipient_token_account.clone());
            }
        }
    }

    require!(
        remaining_index == remaining_accounts.len(),
        PobError::InvalidPumpAccount
    );

    Ok(())
}

fn require_shared_pumpswap_vaults(
    sharing_config: &Pubkey,
    pump_creator_vault: &AccountInfo,
    pump_creator_vault_ata: &AccountInfo,
    coin_creator_vault_authority: &AccountInfo,
    coin_creator_vault_ata: &AccountInfo,
    quote_mint: &Pubkey,
    quote_token_program: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        pump_creator_vault.key(),
        pump_creator_vault_pda(sharing_config).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        pump_creator_vault_ata.key(),
        associated_token_address(pump_creator_vault.key, quote_token_program, quote_mint),
        PobError::InvalidTokenAccount
    );

    let shared_authority = pump_amm_creator_vault_authority_pda(sharing_config).0;
    require_keys_eq!(
        coin_creator_vault_authority.key(),
        shared_authority,
        PobError::InvalidPumpAccount
    );
    require_associated_token_account(
        coin_creator_vault_ata,
        &shared_authority,
        quote_mint,
        quote_token_program,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account_info<'a>(
        key: &'a Pubkey,
        owner: &'a Pubkey,
        lamports: &'a mut u64,
        data: &'a mut [u8],
    ) -> AccountInfo<'a> {
        AccountInfo::new(key, false, true, lamports, data, owner, false, 0)
    }

    #[test]
    fn token_shareholder_accounts_include_wallets_then_token_accounts() {
        let fee_owner = Pubkey::new_unique();
        let other_shareholder = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let quote_token_program = TOKEN_PROGRAM_ID;
        let fee_owner_token_account =
            associated_token_address(&fee_owner, &quote_token_program, &quote_mint);
        let other_token_account =
            associated_token_address(&other_shareholder, &quote_token_program, &quote_mint);
        let owner = system_program::ID;
        let shares = [
            PumpFeeShare { address: fee_owner },
            PumpFeeShare {
                address: other_shareholder,
            },
        ];

        let mut fee_owner_lamports = 1;
        let mut fee_owner_data = [];
        let fee_owner_info = test_account_info(
            &fee_owner,
            &owner,
            &mut fee_owner_lamports,
            &mut fee_owner_data,
        );
        let mut fee_owner_token_lamports = 1;
        let mut fee_owner_token_data = [];
        let fee_owner_token_info = test_account_info(
            &fee_owner_token_account,
            &quote_token_program,
            &mut fee_owner_token_lamports,
            &mut fee_owner_token_data,
        );
        let mut other_lamports = 1;
        let mut other_data = [];
        let other_info = test_account_info(
            &other_shareholder,
            &owner,
            &mut other_lamports,
            &mut other_data,
        );
        let mut other_token_lamports = 1;
        let mut other_token_data = [];
        let other_token_info = test_account_info(
            &other_token_account,
            &quote_token_program,
            &mut other_token_lamports,
            &mut other_token_data,
        );
        let remaining_accounts = [other_info.clone(), other_token_info.clone()];
        let mut accounts = Vec::new();
        let mut infos = Vec::new();

        append_shareholder_accounts(
            &mut accounts,
            &mut infos,
            &shares,
            ShareholderAccountMode::Token {
                fee_owner: fee_owner_info,
                fee_owner_token_account: fee_owner_token_info,
                quote_mint,
                quote_token_program,
            },
            &remaining_accounts,
        )
        .unwrap();

        let account_keys: Vec<Pubkey> = accounts.iter().map(|account| account.pubkey).collect();
        assert_eq!(
            account_keys,
            vec![
                fee_owner,
                other_shareholder,
                fee_owner_token_account,
                other_token_account
            ]
        );
        let info_keys: Vec<Pubkey> = infos.iter().map(|account| account.key()).collect();
        assert_eq!(info_keys, account_keys);
    }
}
