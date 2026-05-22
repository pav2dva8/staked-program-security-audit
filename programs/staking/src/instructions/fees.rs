use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use crate::{
    account_contexts::*, constants::*, errors::PobError, guards::*, pda::*, rewards::*, token::*,
};
pub(crate) fn claim_pump_creator_fees(ctx: Context<ClaimPumpCreatorFees>) -> Result<()> {
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

    apply_rewards_with_protocol_fee(
        &mut ctx.accounts.launch,
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

pub(crate) fn claim_pumpswap_creator_fees(ctx: Context<ClaimPumpSwapCreatorFees>) -> Result<()> {
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

    apply_rewards_with_protocol_fee(
        &mut ctx.accounts.launch,
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
