use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use crate::{
    constants::*, errors::PobError, guards::require_supported_token_program,
    pda::associated_token_address,
};

pub(crate) fn transfer_spl_tokens<'info>(
    token_program: &AccountInfo<'info>,
    source: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    amount: u64,
    expected_token_program: &Pubkey,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    require_keys_eq!(
        token_program.key(),
        *expected_token_program,
        PobError::InvalidTokenAccount
    );

    let mut data = Vec::with_capacity(9);
    data.push(SPL_TOKEN_TRANSFER_IX);
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: *expected_token_program,
        accounts: vec![
            AccountMeta::new(source.key(), false),
            AccountMeta::new(destination.key(), false),
            AccountMeta::new_readonly(authority.key(), true),
        ],
        data,
    };

    invoke_signed(
        &ix,
        &[
            source.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

pub(crate) fn close_spl_token_account<'info>(
    token_program: &AccountInfo<'info>,
    account: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    require_keys_eq!(
        token_program.key(),
        TOKEN_PROGRAM_ID,
        PobError::InvalidTokenAccount
    );

    let ix = Instruction {
        program_id: TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(account.key(), false),
            AccountMeta::new(destination.key(), false),
            AccountMeta::new_readonly(authority.key(), true),
        ],
        data: vec![SPL_TOKEN_CLOSE_ACCOUNT_IX],
    };

    invoke_signed(
        &ix,
        &[
            account.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        signer_seeds,
    )?;

    Ok(())
}

pub(crate) fn require_mint_account(mint: &AccountInfo) -> Result<Pubkey> {
    require_supported_token_program(mint.owner)?;

    let data = mint.try_borrow_data()?;
    require!(data.len() >= MINT_LEN, PobError::InvalidMint);
    require!(data[MINT_IS_INITIALIZED_OFFSET] != 0, PobError::InvalidMint);

    Ok(*mint.owner)
}

pub(crate) fn require_token_account(
    token_account: &AccountInfo,
    mint: &Pubkey,
    authority: &Pubkey,
    token_program: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        *token_account.owner,
        *token_program,
        PobError::InvalidTokenAccount
    );

    let data = token_account.try_borrow_data()?;
    require!(
        data.len() >= TOKEN_ACCOUNT_LEN,
        PobError::InvalidTokenAccount
    );
    require!(
        &data[TOKEN_ACCOUNT_MINT_OFFSET..TOKEN_ACCOUNT_MINT_OFFSET + PUBKEY_BYTES] == mint.as_ref(),
        PobError::InvalidTokenAccount
    );
    require!(
        &data[TOKEN_ACCOUNT_OWNER_OFFSET..TOKEN_ACCOUNT_OWNER_OFFSET + PUBKEY_BYTES]
            == authority.as_ref(),
        PobError::InvalidTokenAccount
    );

    Ok(())
}

pub(crate) fn require_associated_token_account(
    token_account: &AccountInfo,
    authority: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Result<()> {
    require_keys_eq!(
        token_account.key(),
        associated_token_address(authority, token_program, mint),
        PobError::InvalidTokenAccount
    );
    require_token_account(token_account, mint, authority, token_program)
}

pub(crate) fn token_account_amount(token_account: &AccountInfo) -> Result<u64> {
    let data = token_account.try_borrow_data()?;
    require!(
        data.len() >= TOKEN_ACCOUNT_AMOUNT_OFFSET + 8,
        PobError::InvalidTokenAccount
    );

    Ok(u64::from_le_bytes(
        data[TOKEN_ACCOUNT_AMOUNT_OFFSET..TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
            .try_into()
            .map_err(|_| PobError::InvalidTokenAccount)?,
    ))
}

pub(crate) fn read_u32_le(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or(PobError::InvalidProgramAccount)?;
    Ok(u32::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| PobError::InvalidProgramAccount)?,
    ))
}
