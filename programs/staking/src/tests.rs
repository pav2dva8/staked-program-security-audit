use anchor_lang::prelude::*;

use crate::{constants::*, guards::*, pda::*, rewards::*, state::*, token::*};

fn test_launch(total_weighted_stake: u128) -> LaunchConfig {
    LaunchConfig {
        mint: Pubkey::new_from_array([7; 32]),
        token_program: TOKEN_2022_PROGRAM_ID,
        total_weighted_stake,
        acc_reward_per_weight: 0,
        reward_reserve: 0,
    }
}

fn test_position(weight: u128, reward_debt: u128) -> StakePosition {
    StakePosition {
        amount: weight as u64,
        weight,
        unlock_ts: 0,
        reward_debt,
    }
}

fn write_token_account_data(data: &mut [u8], mint: &Pubkey, authority: &Pubkey, amount: u64) {
    data[TOKEN_ACCOUNT_MINT_OFFSET..TOKEN_ACCOUNT_MINT_OFFSET + PUBKEY_BYTES]
        .copy_from_slice(mint.as_ref());
    data[TOKEN_ACCOUNT_OWNER_OFFSET..TOKEN_ACCOUNT_OWNER_OFFSET + PUBKEY_BYTES]
        .copy_from_slice(authority.as_ref());
    data[TOKEN_ACCOUNT_AMOUNT_OFFSET..TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
        .copy_from_slice(&amount.to_le_bytes());
}

#[test]
fn mint_validation_accepts_token_2022_mints() {
    let mint_key = Pubkey::new_unique();
    let mut lamports = 1_000_000;
    let mut data = [0u8; MINT_LEN];
    data[MINT_IS_INITIALIZED_OFFSET] = 1;

    let account = AccountInfo::new(
        &mint_key,
        false,
        false,
        &mut lamports,
        &mut data,
        &TOKEN_2022_PROGRAM_ID,
        false,
        0,
    );

    assert_eq!(
        require_mint_account(&account).unwrap(),
        TOKEN_2022_PROGRAM_ID
    );
}

#[test]
fn lock_multipliers_match_supported_terms() {
    assert_eq!(lock_multiplier_bps(7).unwrap(), 10_000);
    assert_eq!(lock_multiplier_bps(30).unwrap(), 12_500);
    assert_eq!(lock_multiplier_bps(90).unwrap(), 17_500);
    assert_eq!(lock_multiplier_bps(180).unwrap(), 25_000);
    assert!(lock_multiplier_bps(1).is_err());
}

#[test]
fn weighted_amount_uses_basis_points() {
    assert_eq!(weighted_amount(100, 10_000).unwrap(), 100);
    assert_eq!(weighted_amount(100, 12_500).unwrap(), 125);
    assert_eq!(weighted_amount(100, 25_000).unwrap(), 250);
}

#[test]
fn rewards_split_by_weight() {
    let mut launch = test_launch(300);
    let lighter = test_position(100, 0);
    let heavier = test_position(200, 0);

    apply_rewards(&mut launch, 900).unwrap();

    assert_eq!(launch.reward_reserve, 900);
    assert_eq!(pending_reward(&lighter, &launch).unwrap(), 300);
    assert_eq!(pending_reward(&heavier, &launch).unwrap(), 600);
}

#[test]
fn protocol_fee_is_five_percent_rounded_down() {
    assert_eq!(protocol_fee_amount(1_000).unwrap(), 50);
    assert_eq!(protocol_fee_amount(101).unwrap(), 5);
    assert_eq!(protocol_fee_amount(19).unwrap(), 0);
}

#[test]
fn new_position_debt_excludes_prior_rewards() {
    let mut launch = test_launch(100);
    let original = test_position(100, 0);

    apply_rewards(&mut launch, 100).unwrap();
    let joining_debt = reward_debt(100, launch.acc_reward_per_weight).unwrap();
    let joining = test_position(100, joining_debt);
    launch.total_weighted_stake = 200;

    apply_rewards(&mut launch, 100).unwrap();

    assert_eq!(pending_reward(&original, &launch).unwrap(), 150);
    assert_eq!(pending_reward(&joining, &launch).unwrap(), 50);
}

#[test]
fn pump_and_pumpswap_creator_vault_seeds_are_distinct() {
    let creator = Pubkey::new_from_array([9; 32]);

    assert_ne!(
        pump_creator_vault_pda(&creator).0,
        pump_amm_creator_vault_authority_pda(&creator).0
    );
}

#[test]
fn creator_vault_surplus_check_accepts_rent_only_lamports() {
    let rent = Rent::default();

    assert!(!creator_vault_has_fee_surplus(
        rent.minimum_balance(0),
        0,
        &rent
    ));
}

#[test]
fn creator_vault_surplus_check_rejects_fee_surplus() {
    let rent = Rent::default();

    assert!(creator_vault_has_fee_surplus(
        rent.minimum_balance(0) + 1,
        0,
        &rent
    ));
}

#[test]
fn lock_extension_check_rejects_shorter_unlock() {
    assert!(lock_extension_allowed(200, 199).is_err());
}

#[test]
fn lock_extension_check_accepts_same_or_later_unlock() {
    assert!(lock_extension_allowed(200, 200).is_ok());
    assert!(lock_extension_allowed(200, 201).is_ok());
}

#[test]
fn clean_pumpswap_creator_vault_accepts_empty_wsol_vault() {
    let mint = Pubkey::new_from_array([11; 32]);
    let fee_owner = fee_owner_pda(&mint).0;
    let vault_authority = pump_amm_creator_vault_authority_pda(&fee_owner).0;
    let vault_ata = associated_token_address(&vault_authority, &TOKEN_PROGRAM_ID, &NATIVE_MINT_ID);
    let mut lamports = 1_000_000;
    let mut data = [0u8; TOKEN_ACCOUNT_LEN];
    write_token_account_data(&mut data, &NATIVE_MINT_ID, &vault_authority, 0);

    let account = AccountInfo::new(
        &vault_ata,
        false,
        false,
        &mut lamports,
        &mut data,
        &TOKEN_PROGRAM_ID,
        false,
        0,
    );

    assert!(require_clean_pumpswap_creator_vault(&account, &mint).is_ok());
}

#[test]
fn clean_pumpswap_creator_vault_rejects_pending_wsol() {
    let mint = Pubkey::new_from_array([12; 32]);
    let fee_owner = fee_owner_pda(&mint).0;
    let vault_authority = pump_amm_creator_vault_authority_pda(&fee_owner).0;
    let vault_ata = associated_token_address(&vault_authority, &TOKEN_PROGRAM_ID, &NATIVE_MINT_ID);
    let mut lamports = 1_000_000;
    let mut data = [0u8; TOKEN_ACCOUNT_LEN];
    write_token_account_data(&mut data, &NATIVE_MINT_ID, &vault_authority, 1);

    let account = AccountInfo::new(
        &vault_ata,
        false,
        false,
        &mut lamports,
        &mut data,
        &TOKEN_PROGRAM_ID,
        false,
        0,
    );

    assert!(require_clean_pumpswap_creator_vault(&account, &mint).is_err());
}
