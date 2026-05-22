use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct LaunchConfig {
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub total_weighted_stake: u128,
    pub acc_reward_per_weight: u128,
    pub reward_reserve: u64,
}

#[account]
#[derive(InitSpace)]
pub struct StakePosition {
    pub amount: u64,
    pub weight: u128,
    pub unlock_ts: i64,
    pub reward_debt: u128,
}

#[account]
#[derive(InitSpace)]
pub struct QuoteRewardPool {
    pub launch: Pubkey,
    pub quote_mint: Pubkey,
    pub quote_token_program: Pubkey,
    pub acc_reward_per_weight: u128,
    pub reward_reserve: u64,
    pub protocol_fee_reserve: u64,
    pub last_reward_ts: i64,
}

#[account]
#[derive(InitSpace)]
pub struct QuoteStakeState {
    pub stake_position: Pubkey,
    pub reward_pool: Pubkey,
    pub recorded_weight: u128,
    pub reward_debt: u128,
}
