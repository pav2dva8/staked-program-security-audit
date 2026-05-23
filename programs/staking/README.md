# Staking Program

Anchor program package for `staked`, a Solana staking and creator-fee routing program for
Pump.fun-launched tokens.

The program lets holders stake a launched token for a fixed lock period and receive rewards from
creator-fee streams. The fee owner is a program-derived address, so Pump.fun and PumpSwap creator
fees can be claimed only through the bounded staking reward flows implemented here.

Status: in development. This program is unaudited and should not be used in production without an
independent review.

## Program Info

| Field | Value |
| --- | --- |
| Anchor program | `staked` |
| Crate package | `staking-program` |
| Program ID | `aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK` |
| Anchor version | `0.32.1` |
| License | `MIT` |

## Features

- One launch account per mint.
- One `fee_owner` PDA per mint.
- One canonical staking vault ATA per launch.
- One active stake position per wallet per launch.
- Stake increases through the existing position with lock extension checks.
- Fixed-duration stake locks with weighted reward accounting.
- Support for legacy SPL Token and Token-2022 staking mints.
- SOL rewards from Pump.fun bonding-curve creator fees.
- Quote-token rewards from Pump.fun `collect_creator_fee_v2`.
- WSOL rewards from PumpSwap `collect_coin_creator_fee`, unwrapped into SOL.
- Non-WSOL PumpSwap quote-token rewards.
- 5% protocol-fee routing for SOL and quote-token reward streams.
- Protocol-fee claims gated by the program upgrade authority.

## Core Accounts

| Account | Purpose |
| --- | --- |
| `LaunchConfig` | Stores mint, token program, total weighted stake, SOL reward accumulator, and SOL reward reserve. |
| `StakePosition` | Stores a wallet's staked amount, weighted amount, unlock timestamp, and SOL reward debt. |
| `QuoteRewardPool` | Tracks one non-native quote-token reward stream for a launch. |
| `QuoteStakeState` | Tracks a stake position's reward debt for one quote-token reward pool. |

Important PDAs and token accounts:

```text
launch = PDA(["launch", mint])
fee_owner = PDA(["fee-owner", mint])
staking_vault = ATA(launch, token_program, mint)
protocol_fee_vault = PDA(["protocol-fees", mint])

quote_reward_pool = PDA(["quote-rewards", launch, quote_mint])
quote_stake = PDA(["quote-stake", stake_position, quote_reward_pool])
quote_protocol_fee_authority = PDA(["quote-protocol-fees", launch, quote_mint])
quote_reward_vault = ATA(quote_reward_pool, quote_mint)
quote_protocol_fee_vault = ATA(quote_protocol_fee_authority, quote_mint)
```

Pump.fun and PumpSwap integration accounts:

```text
bonding_curve = Pump.fun PDA(["bonding-curve", mint])
creator_vault = Pump.fun PDA(["creator-vault", fee_owner])
sharing_config = Pump Fees PDA(["sharing-config", mint])
shared_creator_vault = Pump.fun PDA(["creator-vault", sharing_config])
pump_event_authority = Pump.fun PDA(["__event_authority"])

coin_creator_vault_authority = PumpSwap PDA(["creator_vault", fee_owner])
coin_creator_vault_ata = ATA(coin_creator_vault_authority, quote_mint)
shared_coin_creator_vault_authority = PumpSwap PDA(["creator_vault", sharing_config])
shared_coin_creator_vault_ata = ATA(shared_coin_creator_vault_authority, quote_mint)
pump_amm_event_authority = PumpSwap PDA(["__event_authority"])
```

## Launch Flow

1. Create or derive the Pump.fun launch mint.
2. Derive `fee_owner = PDA(["fee-owner", mint])`.
3. Configure or verify one supported creator-fee route:
   - Direct route: `bonding_curve.creator == fee_owner`.
   - Pump Fees route: `bonding_curve.creator == sharing_config`, where `sharing_config` is the
     Pump Fees PDA for the mint and includes `fee_owner` as a non-zero shareholder.
4. Create the canonical staking vault ATA for `launch` and `mint`.
5. Call `initialize_launch`. For the Pump Fees route, pass `sharing_config` as the first remaining
   account.

`initialize_launch` validates the mint account, stores the mint's token program, verifies the
staking vault ATA, and checks that the Pump.fun bonding curve uses either the direct route or the
canonical Pump Fees sharing route.

## Staking

Supported lock periods:

| Lock period | Multiplier |
| --- | --- |
| 7 days | 1.00x |
| 30 days | 1.25x |
| 90 days | 1.75x |
| 180 days | 2.50x |

Rewards are distributed by weighted stake:

```text
position_weight = staked_amount * lock_multiplier
reward_share = position_weight / total_weighted_stake
```

`stake` creates the wallet's position. `increase_stake` adds tokens to that existing position,
recalculates weight using the requested lock period, and rejects any change that would shorten the
current unlock timestamp.

After active stake exists, `stake` and `increase_stake` require the Pump.fun creator vault and the
PumpSwap WSOL creator vault to be clean. If creator fees are already waiting, they must be claimed
before new stake weight can join the pool. For Pump Fees routed launches, pass the bonding curve and
sharing config as remaining accounts to `stake` and `increase_stake`, and pass the shared creator
vault accounts instead of the direct `fee_owner` vault accounts.

## Reward Sources

The program has no generic reward-deposit instruction. Rewards come from bounded creator-fee claim
paths:

| Instruction | Source | Reward asset |
| --- | --- | --- |
| `claim_pump_creator_fees` | Pump.fun `collect_creator_fee` | SOL |
| `claim_pump_quote_creator_fees` | Pump.fun `collect_creator_fee_v2` | Non-native quote token |
| `claim_pump_shared_creator_fees` | Pump.fun `distribute_creator_fees_v2` | SOL |
| `claim_pump_shared_quote_creator_fees` | Pump.fun `distribute_creator_fees_v2` | Non-native quote token |
| `claim_pumpswap_creator_fees` | PumpSwap `collect_coin_creator_fee` | WSOL unwrapped into SOL |
| `claim_pumpswap_quote_creator_fees` | PumpSwap `collect_coin_creator_fee` | Non-native quote token |
| `claim_pumpswap_shared_creator_fees` | PumpSwap `transfer_creator_fees_to_pump_v2`, then Pump.fun `distribute_creator_fees_v2` | WSOL unwrapped into SOL |
| `claim_pumpswap_shared_quote_creator_fees` | PumpSwap `transfer_creator_fees_to_pump_v2`, then Pump.fun `distribute_creator_fees_v2` | Non-native quote token |

All creator-fee claim paths require active stake and split out a 5% protocol fee before applying
rewards to stakers.

Shared claim instructions validate the canonical `sharing_config` account, require it to be active,
require shareholder shares to sum to 10,000 bps, and require `fee_owner` to have a non-zero share.
If there are shareholders other than `fee_owner`, pass their writable recipient accounts as remaining
accounts in the same order as the `sharing_config.shareholders` vector. For SOL distributions the
recipient account is the shareholder wallet. For quote-token distributions the recipient account is
the shareholder ATA for `quote_mint` and `quote_token_program`.

SOL rewards are held in the `LaunchConfig` account and paid by `claim_rewards` when no remaining
accounts are supplied. Quote-token rewards use `QuoteRewardPool` and `QuoteStakeState`; to claim a
quote stream, call `claim_rewards` with these remaining accounts in order:

```text
quote_reward_pool
quote_mint
quote_token_program
quote_reward_vault_ata
user_quote_ata
quote_stake
```

## Instructions

| Instruction | Purpose |
| --- | --- |
| `initialize_launch` | Initializes launch state for a Pump.fun mint and verifies the fee-owner setup. |
| `stake` | Creates a fixed-duration stake position. |
| `increase_stake` | Adds tokens to an existing position and optionally extends the lock. |
| `unstake` | Withdraws staked tokens after unlock and pays pending SOL rewards. |
| `claim_rewards` | Claims SOL rewards, or quote-token rewards when quote accounts are passed. |
| `initialize_quote_rewards` | Creates a reward pool for a non-native quote mint. |
| `initialize_quote_stake` | Creates quote reward tracking for one stake position and quote pool. |
| `claim_pump_creator_fees` | Claims Pump.fun SOL creator fees into the launch reward reserve. |
| `claim_pump_quote_creator_fees` | Claims Pump.fun quote-token creator fees into a quote reward pool. |
| `claim_pump_shared_creator_fees` | Distributes shared Pump.fun SOL creator fees and routes `fee_owner`'s share into SOL rewards. |
| `claim_pump_shared_quote_creator_fees` | Distributes shared Pump.fun quote-token creator fees and routes `fee_owner`'s share into a quote reward pool. |
| `claim_pumpswap_creator_fees` | Claims PumpSwap WSOL creator fees and unwraps them into SOL rewards. |
| `claim_pumpswap_quote_creator_fees` | Claims PumpSwap non-native quote-token creator fees. |
| `claim_pumpswap_shared_creator_fees` | Transfers shared PumpSwap WSOL fees into Pump, distributes them, and unwraps `fee_owner`'s share into SOL rewards. |
| `claim_pumpswap_shared_quote_creator_fees` | Transfers shared PumpSwap quote-token fees into Pump, distributes them, and routes `fee_owner`'s share into a quote reward pool. |
| `claim_protocol_fees` | Sends SOL protocol fees to the program upgrade authority. |
| `claim_quote_protocol_fees` | Sends quote-token protocol fees to the program upgrade authority. |

## Security Model

- The `fee_owner` PDA is not initialized as mutable program state. It is only used as a signer for
  specific Pump.fun creator-fee CPIs and as the recipient for creator-fee flows.
- The program does not expose an arbitrary CPI executor.
- Pump.fun and PumpSwap program IDs, event authorities, creator-vault PDAs, and token accounts are
  derived or checked before fee-claim CPIs.
- The launch stores the staking mint's token program so staking works with either SPL Token or
  Token-2022 mints without assuming `Tokenkeg`.
- Reward payouts are bounded by tracked reserves.
- Protocol-fee withdrawals require the current upgrade authority of this program.

See `../../docs/staking-architecture.md` and `../../docs/staking-security-audit.md` for design notes
and known risks.

## Development

From the repository root:

```bash
npm install
anchor build
anchor test
```

For a lighter Rust-only check:

```bash
cargo check
```

For a deterministic Solana program build with Solana Verify:

```bash
npm run install:solana-verify
npm run build:verifiable
npm run hash:verifiable
npm run verify:repo -- --url <RPC_URL> --commit-hash <COMMIT_HASH>
```

Deploy `target/deploy/staked.so` from `npm run build:verifiable` when verifying the deployed
program. A later `anchor build` can change the executable hash.

Devnet helper scripts are available from the root `package.json`:

```bash
npm run create:pump-devnet -- --quote sol
npm run volume:pump-devnet -- --mint <MINT>
npm run stake:test-wallets -- --mint <MINT>
npm run check:scripts
```
