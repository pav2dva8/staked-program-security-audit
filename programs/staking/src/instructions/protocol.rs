use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::{
    account_contexts::*,
    errors::PobError,
    guards::*,
    pda::quote_protocol_fee_authority_pda,
    rewards::protocol_fee_vault_claimable,
    token::{require_associated_token_account, token_account_amount, transfer_spl_tokens},
};

pub(crate) fn claim_quote_protocol_fees(ctx: Context<ClaimQuoteProtocolFees>) -> Result<()> {
    require_program_upgrade_authority(
        &ctx.accounts.program.to_account_info(),
        &ctx.accounts.program_data.to_account_info(),
        &ctx.accounts.authority.key(),
    )?;
    require_quote_reward_pool(
        &ctx.accounts.quote_reward_pool,
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_keys_eq!(
        ctx.accounts.quote_protocol_fee_authority.key(),
        quote_protocol_fee_authority_pda(
            &ctx.accounts.launch.key(),
            &ctx.accounts.quote_mint.key()
        )
        .0,
        PobError::InvalidProtocolFeeVault
    );
    require_associated_token_account(
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_authority.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;
    require_associated_token_account(
        &ctx.accounts.authority_quote_ata.to_account_info(),
        &ctx.accounts.authority.key(),
        &ctx.accounts.quote_mint.key(),
        &ctx.accounts.quote_token_program.key(),
    )?;

    let amount =
        token_account_amount(&ctx.accounts.quote_protocol_fee_vault_ata.to_account_info())?;
    require!(amount > 0, PobError::NoRewards);

    let launch_key = ctx.accounts.launch.key();
    let quote_mint = ctx.accounts.quote_mint.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        b"quote-protocol-fees",
        launch_key.as_ref(),
        quote_mint.as_ref(),
        &[ctx.bumps.quote_protocol_fee_authority],
    ]];
    transfer_spl_tokens(
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.authority_quote_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_authority.to_account_info(),
        amount,
        &ctx.accounts.quote_reward_pool.quote_token_program,
        signer_seeds,
    )?;

    let reward_pool = &mut ctx.accounts.quote_reward_pool;
    reward_pool.protocol_fee_reserve = reward_pool
        .protocol_fee_reserve
        .checked_sub(amount)
        .unwrap_or(0);

    Ok(())
}

pub(crate) fn claim_protocol_fees(ctx: Context<ClaimProtocolFees>) -> Result<()> {
    require_program_upgrade_authority(
        &ctx.accounts.program.to_account_info(),
        &ctx.accounts.program_data.to_account_info(),
        &ctx.accounts.authority.key(),
    )?;
    require_protocol_fee_vault(
        &ctx.accounts.protocol_fee_vault.to_account_info(),
        &ctx.accounts.launch.mint,
    )?;

    let amount = protocol_fee_vault_claimable(&ctx.accounts.protocol_fee_vault.to_account_info())?;
    require!(amount > 0, PobError::NoRewards);

    system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.protocol_fee_vault.to_account_info(),
                to: ctx.accounts.authority.to_account_info(),
            },
            &[&[
                b"protocol-fees",
                ctx.accounts.launch.mint.as_ref(),
                &[ctx.bumps.protocol_fee_vault],
            ]],
        ),
        amount,
    )?;

    Ok(())
}
