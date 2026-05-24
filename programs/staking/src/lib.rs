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

declare_id!("aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK");

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "Staked",
    project_url: "https://github.com/iceypump/staked",
    contacts: "twitter:https://x.com/iceyypump,link:https://github.com/iceypump/staked/security/policy",
    policy: "https://github.com/iceypump/staked/security/policy",
    preferred_languages: "en",
    source_code: "https://github.com/iceypump/staked",
    auditors: "None"
}

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

    pub fn claim_pump_creator_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimPumpCreatorFees<'info>>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pump_creator_fees(ctx)
    }

    pub fn claim_pump_quote_creator_fees(ctx: Context<ClaimPumpQuoteCreatorFees>) -> Result<()> {
        crate::instructions::fees::claim_pump_quote_creator_fees(ctx)
    }

    pub fn claim_pump_shared_creator_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimPumpSharedCreatorFees<'info>>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pump_shared_creator_fees(ctx)
    }

    pub fn claim_pump_shared_quote_creator_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimPumpSharedQuoteCreatorFees<'info>>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pump_shared_quote_creator_fees(ctx)
    }

    pub fn claim_pumpswap_creator_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimPumpSwapCreatorFees<'info>>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pumpswap_creator_fees(ctx)
    }

    pub fn claim_pumpswap_quote_creator_fees(
        ctx: Context<ClaimPumpSwapQuoteCreatorFees>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pumpswap_quote_creator_fees(ctx)
    }

    pub fn claim_pumpswap_shared_creator_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimPumpSwapSharedCreatorFees<'info>>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pumpswap_shared_creator_fees(ctx)
    }

    pub fn claim_pumpswap_shared_quote_creator_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimPumpSwapSharedQuoteCreatorFees<'info>>,
    ) -> Result<()> {
        crate::instructions::fees::claim_pumpswap_shared_quote_creator_fees(ctx)
    }

    pub fn claim_quote_protocol_fees(ctx: Context<ClaimQuoteProtocolFees>) -> Result<()> {
        crate::instructions::protocol::claim_quote_protocol_fees(ctx)
    }

    pub fn claim_protocol_fees(ctx: Context<ClaimProtocolFees>) -> Result<()> {
        crate::instructions::protocol::claim_protocol_fees(ctx)
    }
}
