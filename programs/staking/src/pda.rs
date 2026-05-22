use anchor_lang::prelude::*;

use crate::constants::*;

pub(crate) fn fee_owner_pda(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"fee-owner", mint.as_ref()], &crate::ID)
}

pub(crate) fn protocol_fee_vault_pda(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"protocol-fees", mint.as_ref()], &crate::ID)
}

pub(crate) fn quote_protocol_fee_authority_pda(
    launch: &Pubkey,
    quote_mint: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"quote-protocol-fees", launch.as_ref(), quote_mint.as_ref()],
        &crate::ID,
    )
}

pub(crate) fn pump_bonding_curve_pda(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &PUMP_PROGRAM_ID)
}

pub(crate) fn pump_creator_vault_pda(creator: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &PUMP_PROGRAM_ID)
}

pub(crate) fn pump_event_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"__event_authority"], &PUMP_PROGRAM_ID)
}

pub(crate) fn pump_amm_creator_vault_authority_pda(creator: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"creator_vault", creator.as_ref()], &PUMP_AMM_PROGRAM_ID)
}

pub(crate) fn pump_amm_event_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"__event_authority"], &PUMP_AMM_PROGRAM_ID)
}

pub(crate) fn associated_token_address(
    authority: &Pubkey,
    token_program: &Pubkey,
    mint: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[authority.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}
