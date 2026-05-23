use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;

use crate::{constants::*, errors::PobError, pda::*, state::*, token::*};

#[derive(Clone, Copy)]
pub(crate) struct PumpFeeShare {
    pub address: Pubkey,
}

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

pub(crate) fn require_clean_creator_vault_for_route<'info>(
    creator_vault: &AccountInfo,
    mint: &Pubkey,
    route_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let fee_owner = fee_owner_pda(mint).0;
    if creator_vault.key() == pump_creator_vault_pda(&fee_owner).0 {
        return require_clean_creator_vault(creator_vault, mint);
    }

    let sharing_config = pump_sharing_config_pda(mint).0;
    require_keys_eq!(
        creator_vault.key(),
        pump_creator_vault_pda(&sharing_config).0,
        PobError::InvalidPumpAccount
    );
    require!(route_accounts.len() >= 2, PobError::InvalidPumpAccount);
    require_verified_shared_bonding_curve(
        &route_accounts[0],
        mint,
        &fee_owner,
        &route_accounts[1],
    )?;

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

pub(crate) fn require_clean_pumpswap_creator_vault_for_route<'info>(
    coin_creator_vault_ata: &AccountInfo,
    mint: &Pubkey,
    route_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let fee_owner = fee_owner_pda(mint).0;
    let direct_authority = pump_amm_creator_vault_authority_pda(&fee_owner).0;
    let direct_ata =
        associated_token_address(&direct_authority, &TOKEN_PROGRAM_ID, &NATIVE_MINT_ID);

    if coin_creator_vault_ata.key() == direct_ata {
        return require_clean_pumpswap_creator_vault(coin_creator_vault_ata, mint);
    }

    let sharing_config = pump_sharing_config_pda(mint).0;
    let shared_authority = pump_amm_creator_vault_authority_pda(&sharing_config).0;
    require_keys_eq!(
        coin_creator_vault_ata.key(),
        associated_token_address(&shared_authority, &TOKEN_PROGRAM_ID, &NATIVE_MINT_ID),
        PobError::InvalidPumpAccount
    );
    require!(route_accounts.len() >= 2, PobError::InvalidPumpAccount);
    require_verified_shared_bonding_curve(
        &route_accounts[0],
        mint,
        &fee_owner,
        &route_accounts[1],
    )?;

    if coin_creator_vault_ata.data_is_empty() {
        return Ok(());
    }

    require_token_account(
        coin_creator_vault_ata,
        &NATIVE_MINT_ID,
        &shared_authority,
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

    let creator = pump_bonding_curve_creator(bonding_curve, mint)?;
    require_keys_eq!(creator, *expected_creator, PobError::InvalidPumpAccount);

    Ok(())
}

pub(crate) fn require_pump_creator_route(
    bonding_curve: &AccountInfo,
    mint: &Pubkey,
    fee_owner: &Pubkey,
    sharing_config: Option<&AccountInfo>,
) -> Result<()> {
    let creator = pump_bonding_curve_creator(bonding_curve, mint)?;
    if creator == *fee_owner {
        return Ok(());
    }

    let sharing_config = sharing_config.ok_or(PobError::InvalidPumpAccount)?;
    require_keys_eq!(
        creator,
        pump_sharing_config_pda(mint).0,
        PobError::InvalidPumpAccount
    );
    require_pump_sharing_config(sharing_config, mint, fee_owner)?;

    Ok(())
}

pub(crate) fn require_verified_shared_bonding_curve(
    bonding_curve: &AccountInfo,
    mint: &Pubkey,
    fee_owner: &Pubkey,
    sharing_config: &AccountInfo,
) -> Result<Vec<PumpFeeShare>> {
    let creator = pump_bonding_curve_creator(bonding_curve, mint)?;
    require_keys_eq!(
        creator,
        pump_sharing_config_pda(mint).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(sharing_config.key(), creator, PobError::InvalidPumpAccount);

    require_pump_sharing_config(sharing_config, mint, fee_owner)
}

pub(crate) fn pump_bonding_curve_creator(
    bonding_curve: &AccountInfo,
    mint: &Pubkey,
) -> Result<Pubkey> {
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

    Ok(Pubkey::new_from_array(
        data[PUMP_BONDING_CURVE_CREATOR_OFFSET..PUMP_BONDING_CURVE_CREATOR_OFFSET + PUBKEY_BYTES]
            .try_into()
            .map_err(|_| PobError::InvalidPumpAccount)?,
    ))
}

pub(crate) fn require_pump_sharing_config(
    sharing_config: &AccountInfo,
    mint: &Pubkey,
    fee_owner: &Pubkey,
) -> Result<Vec<PumpFeeShare>> {
    require_keys_eq!(
        sharing_config.key(),
        pump_sharing_config_pda(mint).0,
        PobError::InvalidPumpAccount
    );
    require_keys_eq!(
        *sharing_config.owner,
        PUMP_FEES_PROGRAM_ID,
        PobError::InvalidPumpAccount
    );

    let data = sharing_config.try_borrow_data()?;
    require!(
        data.len() >= PUMP_FEES_SHARING_CONFIG_SHAREHOLDERS_OFFSET + 4,
        PobError::InvalidPumpAccount
    );
    require!(
        &data[..8] == PUMP_FEES_SHARING_CONFIG_DISCRIMINATOR.as_ref(),
        PobError::InvalidPumpAccount
    );
    require!(
        data[PUMP_FEES_SHARING_CONFIG_STATUS_OFFSET] == PUMP_FEES_SHARING_CONFIG_ACTIVE_STATUS,
        PobError::InvalidPumpAccount
    );
    require!(
        &data[PUMP_FEES_SHARING_CONFIG_MINT_OFFSET
            ..PUMP_FEES_SHARING_CONFIG_MINT_OFFSET + PUBKEY_BYTES]
            == mint.as_ref(),
        PobError::InvalidPumpAccount
    );

    let share_count =
        read_u32_le_pump(&data, PUMP_FEES_SHARING_CONFIG_SHAREHOLDERS_OFFSET)? as usize;
    let shares_offset = PUMP_FEES_SHARING_CONFIG_SHAREHOLDERS_OFFSET + 4;
    let shares_len = share_count
        .checked_mul(PUMP_FEES_SHAREHOLDER_LEN)
        .ok_or(PobError::MathOverflow)?;
    require!(
        data.len() >= shares_offset + shares_len,
        PobError::InvalidPumpAccount
    );

    let mut shares = Vec::with_capacity(share_count);
    let mut has_fee_owner_share = false;
    let mut total_share_bps = 0u32;
    for index in 0..share_count {
        let offset = shares_offset + (index * PUMP_FEES_SHAREHOLDER_LEN);
        let address = Pubkey::new_from_array(
            data[offset..offset + PUBKEY_BYTES]
                .try_into()
                .map_err(|_| PobError::InvalidPumpAccount)?,
        );
        let share_bps = u16::from_le_bytes(
            data[offset + PUBKEY_BYTES..offset + PUBKEY_BYTES + 2]
                .try_into()
                .map_err(|_| PobError::InvalidPumpAccount)?,
        );
        require!(share_bps > 0, PobError::InvalidPumpAccount);
        total_share_bps = total_share_bps
            .checked_add(u32::from(share_bps))
            .ok_or(PobError::MathOverflow)?;
        if address == *fee_owner && share_bps > 0 {
            has_fee_owner_share = true;
        }
        shares.push(PumpFeeShare { address });
    }

    require!(has_fee_owner_share, PobError::InvalidPumpAccount);
    require!(
        total_share_bps == BPS_DENOMINATOR as u32,
        PobError::InvalidPumpAccount
    );

    Ok(shares)
}

fn read_u32_le_pump(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or(PobError::InvalidPumpAccount)?;
    Ok(u32::from_le_bytes(
        bytes.try_into().map_err(|_| PobError::InvalidPumpAccount)?,
    ))
}
