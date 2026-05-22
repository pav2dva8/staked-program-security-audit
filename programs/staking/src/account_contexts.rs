use anchor_lang::prelude::*;

use crate::state::*;

#[derive(Accounts)]
pub struct InitializeLaunch<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Validated as an initialized SPL Token or Token-2022 mint.
    pub mint: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + LaunchConfig::INIT_SPACE,
        seeds = [b"launch", mint.key().as_ref()],
        bump
    )]
    pub launch: Account<'info, LaunchConfig>,
    /// CHECK: Validated as Pump.fun bonding curve PDA for mint with fee_owner as creator.
    pub bonding_curve: UncheckedAccount<'info>,
    /// CHECK: Program-derived creator account checked through bonding_curve.creator.
    #[account(seeds = [b"fee-owner", mint.key().as_ref()], bump)]
    pub fee_owner: UncheckedAccount<'info>,
    /// CHECK: Validated as the canonical launch ATA for this mint.
    pub staking_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,
    #[account(
        mut,
        seeds = [b"launch", launch.mint.as_ref()],
        bump
    )]
    pub launch: Account<'info, LaunchConfig>,
    /// CHECK: Validated as staker-owned SPL Token account for mint.
    #[account(mut)]
    pub user_token: UncheckedAccount<'info>,
    /// CHECK: Validated as Pump.fun creator-vault PDA and required to be empty before staking.
    pub creator_vault: UncheckedAccount<'info>,
    /// CHECK: Validated as PumpSwap creator-vault WSOL ATA and required to be empty before staking.
    pub coin_creator_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as the canonical launch ATA for this mint.
    #[account(mut)]
    pub staking_vault: UncheckedAccount<'info>,
    #[account(
        init,
        payer = staker,
        space = 8 + StakePosition::INIT_SPACE,
        seeds = [
            b"stake",
            launch.key().as_ref(),
            staker.key().as_ref()
        ],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,
    /// CHECK: Must match the launch mint's token program.
    pub token_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct IncreaseStake<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,
    #[account(
        mut,
        seeds = [b"launch", launch.mint.as_ref()],
        bump
    )]
    pub launch: Account<'info, LaunchConfig>,
    /// CHECK: Validated as staker-owned SPL Token account for mint.
    #[account(mut)]
    pub user_token: UncheckedAccount<'info>,
    /// CHECK: Validated as Pump.fun creator-vault PDA and required to be empty before adding stake.
    pub creator_vault: UncheckedAccount<'info>,
    /// CHECK: Validated as PumpSwap creator-vault WSOL ATA and required to be empty before adding stake.
    pub coin_creator_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as the canonical launch ATA for this mint.
    #[account(mut)]
    pub staking_vault: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            b"stake",
            launch.key().as_ref(),
            staker.key().as_ref()
        ],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,
    /// CHECK: Must match the launch mint's token program.
    pub token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut, seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(
        mut,
        seeds = [b"stake", launch.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,
}

#[derive(Accounts)]
pub struct InitializeQuoteRewards<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(
        init,
        payer = payer,
        space = 8 + QuoteRewardPool::INIT_SPACE,
        seeds = [
            b"quote-rewards",
            launch.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump
    )]
    pub quote_reward_pool: Account<'info, QuoteRewardPool>,
    /// CHECK: Validated as initialized SPL Token mint.
    pub quote_mint: UncheckedAccount<'info>,
    /// CHECK: Must match quote_mint.owner.
    pub quote_token_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeQuoteStake<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(
        seeds = [
            b"quote-rewards",
            launch.key().as_ref(),
            quote_reward_pool.quote_mint.as_ref()
        ],
        bump
    )]
    pub quote_reward_pool: Account<'info, QuoteRewardPool>,
    #[account(
        seeds = [b"stake", launch.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,
    /// CHECK: Only used as the stake-position seed owner.
    pub owner: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + QuoteStakeState::INIT_SPACE,
        seeds = [
            b"quote-stake",
            stake_position.key().as_ref(),
            quote_reward_pool.key().as_ref()
        ],
        bump
    )]
    pub quote_stake: Account<'info, QuoteStakeState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"launch", launch.mint.as_ref()],
        bump
    )]
    pub launch: Account<'info, LaunchConfig>,
    /// CHECK: Validated as the canonical launch ATA for this mint.
    #[account(mut)]
    pub staking_vault: UncheckedAccount<'info>,
    /// CHECK: Validated as owner-owned SPL Token account for mint.
    #[account(mut)]
    pub user_token: UncheckedAccount<'info>,
    #[account(
        mut,
        close = owner,
        seeds = [b"stake", launch.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,
    /// CHECK: Must match the launch mint's token program.
    pub token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ClaimPumpCreatorFees<'info> {
    #[account(mut, seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Validated as Pump.fun bonding curve PDA for launch.mint with fee_owner as creator.
    pub bonding_curve: UncheckedAccount<'info>,
    /// CHECK: Program-derived creator account used only as a Pump.fun signer.
    #[account(mut, seeds = [b"fee-owner", launch.mint.as_ref()], bump)]
    pub fee_owner: UncheckedAccount<'info>,
    /// CHECK: Validated against Pump.fun creator-vault PDA seeds.
    #[account(mut)]
    pub creator_vault: UncheckedAccount<'info>,
    /// CHECK: Validated as PDA(["protocol-fees", launch.mint]) and stores SOL protocol fees.
    #[account(mut, seeds = [b"protocol-fees", launch.mint.as_ref()], bump)]
    pub protocol_fee_vault: UncheckedAccount<'info>,
    /// CHECK: Validated against Pump.fun event-authority PDA seeds.
    pub pump_event_authority: UncheckedAccount<'info>,
    /// CHECK: Must be the official Pump.fun program id.
    pub pump_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimPumpQuoteCreatorFees<'info> {
    #[account(mut, seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Validated as Pump.fun bonding curve PDA for launch.mint with fee_owner as creator.
    pub bonding_curve: UncheckedAccount<'info>,
    /// CHECK: Program-derived creator account used as a Pump.fun signer.
    #[account(mut, seeds = [b"fee-owner", launch.mint.as_ref()], bump)]
    pub fee_owner: UncheckedAccount<'info>,
    /// CHECK: Validated against Pump.fun creator-vault PDA seeds.
    #[account(mut)]
    pub creator_vault: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for creator_vault.
    #[account(mut)]
    pub creator_vault_token_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for fee_owner.
    #[account(mut)]
    pub fee_owner_token_ata: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            b"quote-rewards",
            launch.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump
    )]
    pub quote_reward_pool: Account<'info, QuoteRewardPool>,
    /// CHECK: Validated as canonical quote ATA for quote_reward_pool.
    #[account(mut)]
    pub quote_reward_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as quote-protocol-fees PDA.
    #[account(
        seeds = [
            b"quote-protocol-fees",
            launch.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump
    )]
    pub quote_protocol_fee_authority: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for quote_protocol_fee_authority.
    #[account(mut)]
    pub quote_protocol_fee_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated against quote_reward_pool.
    pub quote_mint: UncheckedAccount<'info>,
    /// CHECK: Validated against quote_reward_pool.
    pub quote_token_program: UncheckedAccount<'info>,
    /// CHECK: Must be the associated token program.
    pub associated_token_program: UncheckedAccount<'info>,
    /// CHECK: Validated against Pump.fun event-authority PDA seeds.
    pub pump_event_authority: UncheckedAccount<'info>,
    /// CHECK: Must be the official Pump.fun program id.
    pub pump_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimPumpSwapCreatorFees<'info> {
    #[account(mut, seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Validated as PDA(["protocol-fees", launch.mint]) and stores SOL protocol fees.
    #[account(mut, seeds = [b"protocol-fees", launch.mint.as_ref()], bump)]
    pub protocol_fee_vault: UncheckedAccount<'info>,
    /// CHECK: Must be the native SOL mint because rewards are paid as SOL.
    pub quote_mint: UncheckedAccount<'info>,
    /// CHECK: Must be the standard SPL Token program for WSOL.
    pub quote_token_program: UncheckedAccount<'info>,
    /// CHECK: Program-derived creator account used as the PumpSwap coin creator.
    #[account(mut, seeds = [b"fee-owner", launch.mint.as_ref()], bump)]
    pub fee_owner: UncheckedAccount<'info>,
    /// CHECK: Validated against PumpSwap creator_vault PDA seeds.
    pub coin_creator_vault_authority: UncheckedAccount<'info>,
    /// CHECK: Validated as the canonical WSOL ATA for the PumpSwap creator vault authority.
    #[account(mut)]
    pub coin_creator_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as the canonical WSOL ATA for fee_owner, then closed into launch.
    #[account(mut)]
    pub fee_owner_wsol_ata: UncheckedAccount<'info>,
    /// CHECK: Validated against PumpSwap event-authority PDA seeds.
    pub pump_amm_event_authority: UncheckedAccount<'info>,
    /// CHECK: Must be the official PumpSwap AMM program id.
    pub pump_amm_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimPumpSwapQuoteCreatorFees<'info> {
    #[account(mut, seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Program-derived creator account used as the PumpSwap coin creator.
    #[account(mut, seeds = [b"fee-owner", launch.mint.as_ref()], bump)]
    pub fee_owner: UncheckedAccount<'info>,
    /// CHECK: Validated against PumpSwap creator_vault PDA seeds.
    pub coin_creator_vault_authority: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for PumpSwap creator vault authority.
    #[account(mut)]
    pub coin_creator_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for fee_owner.
    #[account(mut)]
    pub fee_owner_token_ata: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            b"quote-rewards",
            launch.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump
    )]
    pub quote_reward_pool: Account<'info, QuoteRewardPool>,
    /// CHECK: Validated as canonical quote ATA for quote_reward_pool.
    #[account(mut)]
    pub quote_reward_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as quote-protocol-fees PDA.
    #[account(
        seeds = [
            b"quote-protocol-fees",
            launch.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump
    )]
    pub quote_protocol_fee_authority: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for quote_protocol_fee_authority.
    #[account(mut)]
    pub quote_protocol_fee_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated against quote_reward_pool.
    pub quote_mint: UncheckedAccount<'info>,
    /// CHECK: Validated against quote_reward_pool.
    pub quote_token_program: UncheckedAccount<'info>,
    /// CHECK: Validated against PumpSwap event-authority PDA seeds.
    pub pump_amm_event_authority: UncheckedAccount<'info>,
    /// CHECK: Must be the official PumpSwap AMM program id.
    pub pump_amm_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ClaimQuoteProtocolFees<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    #[account(
        mut,
        seeds = [
            b"quote-rewards",
            launch.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump
    )]
    pub quote_reward_pool: Account<'info, QuoteRewardPool>,
    /// CHECK: Validated as quote-protocol-fees PDA.
    #[account(
        seeds = [
            b"quote-protocol-fees",
            launch.key().as_ref(),
            quote_mint.key().as_ref()
        ],
        bump
    )]
    pub quote_protocol_fee_authority: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for quote_protocol_fee_authority.
    #[account(mut)]
    pub quote_protocol_fee_vault_ata: UncheckedAccount<'info>,
    /// CHECK: Validated as canonical quote ATA for authority.
    #[account(mut)]
    pub authority_quote_ata: UncheckedAccount<'info>,
    /// CHECK: Validated against quote_reward_pool.
    pub quote_mint: UncheckedAccount<'info>,
    /// CHECK: Validated against quote_reward_pool.
    pub quote_token_program: UncheckedAccount<'info>,
    /// CHECK: Must be this executable program account.
    pub program: UncheckedAccount<'info>,
    /// CHECK: Must be this program's ProgramData account and authority must match signer.
    pub program_data: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ClaimProtocolFees<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [b"launch", launch.mint.as_ref()], bump)]
    pub launch: Account<'info, LaunchConfig>,
    /// CHECK: Validated as PDA(["protocol-fees", launch.mint]) and used as signed SOL vault.
    #[account(
        mut,
        seeds = [b"protocol-fees", launch.mint.as_ref()],
        bump
    )]
    pub protocol_fee_vault: UncheckedAccount<'info>,
    /// CHECK: Must be this executable program account.
    pub program: UncheckedAccount<'info>,
    /// CHECK: Must be this program's ProgramData account and authority must match signer.
    pub program_data: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
