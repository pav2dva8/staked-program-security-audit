use anchor_lang::prelude::*;

mod account_contexts;
mod constants;
mod errors;
mod guards;
mod pda;
mod rewards;
mod state;
mod token;

mod instructions;

#[cfg(test)]
mod tests;

use account_contexts::*;

declare_id!("3skopQVdqns5x5GjU2c3S4nEcmVbDTkMoZWRaVLsJrAa");

#[program]
pub mod staked {
    use super::*;

    pub fn initialize_launch(ctx: Context<InitializeLaunch>) -> Result<()> {
        crate::instructions::staking::initialize_launch(ctx)
    }

    pub fn stake(ctx: Context<Stake>, amount: u64, lock_days: u16) -> Result<()> {
        crate::instructions::staking::stake(ctx, amount, lock_days)
    }

    pub fn increase_stake(ctx: Context<IncreaseStake>, amount: u64, lock_days: u16) -> Result<()> {
        crate::instructions::staking::increase_stake(ctx, amount, lock_days)
    }

    pub fn claim_rewards<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimRewards<'info>>,
    ) -> Result<()> {
        crate::instructions::rewards::claim_rewards(ctx)
    }

    pub fn initialize_quote_rewards(ctx: Context<InitializeQuoteRewards>) -> Result<()> {
        crate::instructions::rewards::initialize_quote_rewards(ctx)
    }

    pub fn initialize_quote_stake(ctx: Context<InitializeQuoteStake>) -> Result<()> {
        crate::instructions::rewards::initialize_quote_stake(ctx)
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        crate::instructions::staking::unstake(ctx)
    }

    pub fn claim_pump_creator_fees(ctx: Context<ClaimPumpCreatorFees>) -> Result<()> {
        crate::instructions::fees::claim_pump_creator_fees(ctx)
    }

    pub fn claim_pump_quote_creator_fees(ctx: Context<ClaimPumpQuoteCreatorFees>) -> Result<()> {
        crate::instructions::fees::claim_pump_quote_creator_fees(ctx)
    }

    pub fn claim_pumpswap_creator_fees(ctx: Context<ClaimPumpSwapCreatorFees>) -> Result<()> {
        crate::instructions::fees::claim_pumpswap_creator_fees(ctx)
    }

    pub fn claim_pumpswap_quote_creator_fees(
        ctx: Context<ClaimPumpSwapQuoteCreatorFees>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pumpswap_quote_creator_fees(ctx)
    }

    pub fn claim_quote_protocol_fees(ctx: Context<ClaimQuoteProtocolFees>) -> Result<()> {
        crate::instructions::protocol::claim_quote_protocol_fees(ctx)
    }

    pub fn claim_protocol_fees(ctx: Context<ClaimProtocolFees>) -> Result<()> {
        crate::instructions::protocol::claim_protocol_fees(ctx)
    }
}
