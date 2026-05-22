use anchor_lang::prelude::*;

pub(crate) const ACC_REWARD_PRECISION: u128 = 1_000_000_000_000_000_000;
pub(crate) const BPS_DENOMINATOR: u128 = 10_000;
pub(crate) const PROTOCOL_FEE_BPS: u64 = 500;
pub(crate) const DAY_SECONDS: i64 = 86_400;
pub(crate) const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub(crate) const PUMP_PROGRAM_ID: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
pub(crate) const PUMP_AMM_PROGRAM_ID: Pubkey =
    pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
pub(crate) const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub(crate) const TOKEN_2022_PROGRAM_ID: Pubkey =
    pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub(crate) const NATIVE_MINT_ID: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
pub(crate) const SPL_TOKEN_TRANSFER_IX: u8 = 3;
pub(crate) const SPL_TOKEN_CLOSE_ACCOUNT_IX: u8 = 9;
pub(crate) const PUMP_COLLECT_CREATOR_FEE_IX: [u8; 8] = [20, 22, 86, 123, 198, 28, 219, 132];
pub(crate) const PUMP_COLLECT_CREATOR_FEE_V2_IX: [u8; 8] = [207, 17, 138, 242, 4, 34, 19, 56];
pub(crate) const PUMP_AMM_COLLECT_COIN_CREATOR_FEE_IX: [u8; 8] =
    [160, 57, 89, 42, 181, 139, 43, 66];
pub(crate) const PUMP_BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
pub(crate) const PUMP_BONDING_CURVE_CREATOR_OFFSET: usize = 8 + (5 * 8) + 1;
pub(crate) const MINT_LEN: usize = 82;
pub(crate) const MINT_IS_INITIALIZED_OFFSET: usize = 45;
pub(crate) const TOKEN_ACCOUNT_LEN: usize = 165;
pub(crate) const TOKEN_ACCOUNT_MINT_OFFSET: usize = 0;
pub(crate) const TOKEN_ACCOUNT_OWNER_OFFSET: usize = 32;
pub(crate) const TOKEN_ACCOUNT_AMOUNT_OFFSET: usize = 64;
pub(crate) const PUBKEY_BYTES: usize = 32;
