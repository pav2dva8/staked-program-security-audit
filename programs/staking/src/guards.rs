use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;

use crate::{constants::*, errors::PobError, pda::*, state::*, token::*};

pub(crate) fn require_canonical_staking_vault(
    staking_vault: &Pubkey,
    launch: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        *staking_vault,
        associated_token_address(launch, token_program, mint),
        PobError::InvalidStakingVault
    );

    Ok(())
}

pub(crate) fn require_supported_token_program(token_program: &Pubkey) -> Result<()> {
    require!(
        *token_program == TOKEN_PROGRAM_ID || *token_program == TOKEN_2022_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );

    Ok(())
}

pub(crate) fn require_protocol_fee_vault(
    protocol_fee_vault: &AccountInfo,
    mint: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        protocol_fee_vault.key(),
        protocol_fee_vault_pda(mint).0,
        PobError::InvalidProtocolFeeVault
    );
    require_keys_eq!(
        *protocol_fee_vault.owner,
        system_program::ID,
        PobError::InvalidProtocolFeeVault
    );
    require!(
        protocol_fee_vault.data_is_empty(),
        PobError::InvalidProtocolFeeVault
    );

    Ok(())
}

pub(crate) fn require_quote_reward_pool(
    reward_pool: &QuoteRewardPool,
    launch: &Pubkey,
    quote_mint: &Pubkey,
    quote_token_program: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        reward_pool.launch,
        *launch,
        PobError::InvalidProtocolFeeVault
    );
    require_keys_eq!(reward_pool.quote_mint, *quote_mint, PobError::InvalidMint);
    require_keys_eq!(
        reward_pool.quote_token_program,
        *quote_token_program,
        PobError::InvalidTokenAccount
    );
    require_supported_token_program(quote_token_program)?;
    Ok(())
}

pub(crate) fn require_quote_stake(
    quote_stake: &QuoteStakeState,
    stake_position: &Pubkey,
    reward_pool: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        quote_stake.stake_position,
        *stake_position,
        PobError::InvalidStakingVault
    );
    require_keys_eq!(
        quote_stake.reward_pool,
        *reward_pool,
        PobError::InvalidProtocolFeeVault
    );
    Ok(())
}

pub(crate) fn require_quote_fee_accounts(
    fee_owner: &Pubkey,
    reward_pool: &Pubkey,
    quote_protocol_fee_authority: &Pubkey,
    fee_owner_token_ata: &AccountInfo,
    quote_reward_vault_ata: &AccountInfo,
    quote_protocol_fee_vault_ata: &AccountInfo,
    launch: &Pubkey,
    quote_mint: &Pubkey,
    quote_token_program: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        *quote_protocol_fee_authority,
        quote_protocol_fee_authority_pda(launch, quote_mint).0,
        PobError::InvalidProtocolFeeVault
    );
    require_associated_token_account(
        fee_owner_token_ata,
        fee_owner,
        quote_mint,
        quote_token_program,
    )?;
    require_associated_token_account(
        quote_reward_vault_ata,
        reward_pool,
        quote_mint,
        quote_token_program,
    )?;
    require_associated_token_account(
        quote_protocol_fee_vault_ata,
        quote_protocol_fee_authority,
        quote_mint,
        quote_token_program,
    )
}

pub(crate) fn require_program_upgrade_authority(
    program: &AccountInfo,
    program_data: &AccountInfo,
    authority: &Pubkey,
) -> Result<()> {
    require_keys_eq!(program.key(), crate::ID, PobError::InvalidProgramAccount);
    require_keys_eq!(
        *program.owner,
        bpf_loader_upgradeable::id(),
        PobError::InvalidProgramAccount
    );
    require!(program.executable, PobError::InvalidProgramAccount);

    let program_account_data = program.try_borrow_data()?;
    require!(
        program_account_data.len() >= 36,
        PobError::InvalidProgramAccount
    );
    require!(
        read_u32_le(&program_account_data, 0)? == 2,
        PobError::InvalidProgramAccount
    );
    let programdata_address = Pubkey::new_from_array(
        program_account_data[4..36]
            .try_into()
            .map_err(|_| PobError::InvalidProgramAccount)?,
    );

    require_keys_eq!(
        program_data.key(),
        programdata_address,
        PobError::InvalidProgramAccount
    );
    require_keys_eq!(
        *program_data.owner,
        bpf_loader_upgradeable::id(),
        PobError::InvalidProgramAccount
    );

    let program_data_account_data = program_data.try_borrow_data()?;
    require!(
        program_data_account_data.len() >= 13,
        PobError::InvalidProgramAccount
    );
    require!(
        read_u32_le(&program_data_account_data, 0)? == 3,
        PobError::InvalidProgramAccount
    );
    require!(
        program_data_account_data[12] == 1,
        PobError::UnauthorizedProtocolFeeClaim
    );
    require!(
        program_data_account_data.len() >= 45,
        PobError::InvalidProgramAccount
    );
    let upgrade_authority = Pubkey::new_from_array(
        program_data_account_data[13..45]
            .try_into()
            .map_err(|_| PobError::InvalidProgramAccount)?,
    );

    require_keys_eq!(
        *authority,
        upgrade_authority,
        PobError::UnauthorizedProtocolFeeClaim
    );

    Ok(())
}

pub(crate) fn require_clean_creator_vault(
    creator_vault: &AccountInfo,
    mint: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        creator_vault.key(),
        pump_creator_vault_pda(&fee_owner_pda(mint).0).0,
        PobError::InvalidPumpAccount
    );
    let rent = Rent::get()?;
    require!(
        !creator_vault_has_fee_surplus(creator_vault.lamports(), creator_vault.data_len(), &rent),
        PobError::UnclaimedCreatorFees
    );

    Ok(())
}

pub(crate) fn creator_vault_has_fee_surplus(lamports: u64, data_len: usize, rent: &Rent) -> bool {
    lamports > rent.minimum_balance(data_len)
}

pub(crate) fn require_clean_pumpswap_creator_vault(
    coin_creator_vault_ata: &AccountInfo,
    mint: &Pubkey,
) -> Result<()> {
    let fee_owner = fee_owner_pda(mint).0;
    let coin_creator_vault_authority = pump_amm_creator_vault_authority_pda(&fee_owner).0;

    require_keys_eq!(
        coin_creator_vault_ata.key(),
        associated_token_address(
            &coin_creator_vault_authority,
            &TOKEN_PROGRAM_ID,
            &NATIVE_MINT_ID,
        ),
        PobError::InvalidPumpAccount
    );

    if coin_creator_vault_ata.data_is_empty() {
        return Ok(());
    }

    require_token_account(
        coin_creator_vault_ata,
        &NATIVE_MINT_ID,
        &coin_creator_vault_authority,
        &TOKEN_PROGRAM_ID,
    )?;
    require!(
        token_account_amount(coin_creator_vault_ata)? == 0,
        PobError::UnclaimedCreatorFees
    );

    Ok(())
}

pub(crate) fn require_verified_bonding_curve(
    bonding_curve: &AccountInfo,
    mint: &Pubkey,
    expected_creator: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        bonding_curve.key(),
        pump_bonding_curve_pda(mint).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        *bonding_curve.owner,
        PUMP_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );

    let data = bonding_curve.try_borrow_data()?;
    require!(
        data.len() >= PUMP_BONDING_CURVE_CREATOR_OFFSET + PUBKEY_BYTES,
        PobError::InvalidPumpAccount
    );
    require!(
        &data[..8] == PUMP_BONDING_CURVE_DISCRIMINATOR.as_ref(),
        PobError::InvalidPumpAccount
    );

    let creator = Pubkey::new_from_array(
        data[PUMP_BONDING_CURVE_CREATOR_OFFSET..PUMP_BONDING_CURVE_CREATOR_OFFSET + PUBKEY_BYTES]
            .try_into()
            .map_err(|_| PobError::InvalidPumpAccount)?,
    );
    require_keys_eq!(creator, *expected_creator, PobError::InvalidPumpAccount);

    Ok(())
}
