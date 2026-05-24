# Staked Program — INFO-02: Permissionless `initialize_launch` and `initialize_quote_rewards`

**Document version:** 1.0
**Date:** 2026-05-24
**Program:** `staked`
**Program ID:** `aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK`
**Severity:** **Informational** (design observation)
**Status:** Likely intentional. Worth confirming.

---

## Table of contents

1. [Executive summary](#executive-summary)
2. [Severity and intent](#severity-and-intent)
3. [Affected scope](#affected-scope)
4. [Authority surfaces](#authority-surfaces)
5. [Vulnerable / behavioural code paths](#vulnerable--behavioural-code-paths)
6. [What a non-creator can do](#what-a-non-creator-can-do)
7. [What a non-creator cannot do](#what-a-non-creator-cannot-do)
8. [Threat scenarios](#threat-scenarios)
9. [Recommended actions](#recommended-actions)
10. [References](#references)

---

## Executive summary

Two creation paths in the program are permissionless:

1. `initialize_launch` — anyone can call it for any Pump.fun mint, provided the
   bonding curve's `creator` field already points at this program's `fee_owner`
   PDA (or at a Pump Fees `sharing_config` PDA that lists the `fee_owner` as a
   shareholder).
2. `initialize_quote_rewards` — anyone can call it for any `(launch, quote_mint)`
   pair, creating a `QuoteRewardPool` PDA.

Neither requires the Pump.fun launch creator's signature, the program upgrade
authority's signature, or any allow-listed wallet. This is consistent with the
"creators opt in by routing the bonding curve to the program PDA" design model.
The effective access control is **upstream** — at Pump.fun, when the bonding
curve is set up.

This document confirms the access-control surface and documents the resulting
threat model so the team can verify it matches intent.

---

## Severity and intent

| Attribute | Assessment |
|-----------|------------|
| **Severity** | Informational |
| **Intent** | Likely deliberate — supports a creator-opt-in model where Pump.fun is the gating layer |
| **Funds at risk** | None directly from these instructions |
| **Concern** | Front-running of launch PDAs; PDA squatting on `QuoteRewardPool` for unwanted quote mints |

---

## Affected scope

### Instructions

| Instruction | Permissionlessness |
|-------------|--------------------|
| `initialize_launch` | Anyone with the rent-fund can call. Gating is the on-chain Pump.fun bonding-curve creator check. |
| `initialize_quote_rewards` | Anyone with the rent-fund can call. Gating is the mint-owner check (must be a real SPL Token / Token-2022 mint, and must not be the native SOL mint). |

### Not in scope here

- `initialize_quote_stake` (also permissionless, but bound to an existing
  `StakePosition` PDA) — addressed indirectly under MED-01.
- `claim_protocol_fees` / `claim_quote_protocol_fees` — gated by program upgrade
  authority via `require_program_upgrade_authority`.

---

## Authority surfaces

The program enforces no allow-list. Its access controls are:

1. **`fee_owner` PDA = `["fee-owner", mint]`** — this PDA only signs CPIs from
   this program. It can only collect fees if Pump.fun's bonding-curve creator
   field points at it. Hence the gating is at Pump.fun.
2. **`require_pump_creator_route`** in `initialize_launch` — verifies that the
   bonding curve's creator is either the `fee_owner` PDA directly, or a
   `sharing_config` PDA that lists the `fee_owner` as a shareholder.
3. **`require_mint_account`** in `initialize_quote_rewards` — verifies the mint
   is a valid SPL Token or Token-2022 mint and not the native SOL mint
   (`require_keys_neq!(quote_mint.key(), NATIVE_MINT_ID, ...)`).

---

## Vulnerable / behavioural code paths

### `initialize_launch`

```10:40:programs/staking/src/instructions/staking.rs
pub(crate) fn initialize_launch(ctx: Context<InitializeLaunch>) -> Result<()> {
    let token_program = require_mint_account(&ctx.accounts.mint.to_account_info())?;
    require_canonical_staking_vault(/* ... */)?;
    require_token_account(/* staking_vault belongs to launch PDA */)?;
    let fee_owner = ctx.accounts.fee_owner.key();
    require_pump_creator_route(
        &ctx.accounts.bonding_curve.to_account_info(),
        &ctx.accounts.mint.key(),
        &fee_owner,
        ctx.remaining_accounts.first(),
    )?;

    let launch = &mut ctx.accounts.launch;
    launch.mint = ctx.accounts.mint.key();
    launch.token_program = token_program;
    launch.total_weighted_stake = 0;
    launch.acc_reward_per_weight = 0;
    launch.reward_reserve = 0;

    Ok(())
}
```

The only signer is `payer`. There is no `creator: Signer` or upgrade authority
check.

### `initialize_quote_rewards`

```30:53:programs/staking/src/instructions/rewards.rs
pub(crate) fn initialize_quote_rewards(ctx: Context<InitializeQuoteRewards>) -> Result<()> {
    let quote_token_program = require_mint_account(&ctx.accounts.quote_mint.to_account_info())?;
    require_keys_eq!(
        ctx.accounts.quote_token_program.key(),
        quote_token_program,
        PobError::InvalidTokenAccount
    );
    require_keys_neq!(
        ctx.accounts.quote_mint.key(),
        NATIVE_MINT_ID,
        PobError::InvalidPumpAccount
    );

    let reward_pool = &mut ctx.accounts.quote_reward_pool;
    reward_pool.launch = ctx.accounts.launch.key();
    reward_pool.quote_mint = ctx.accounts.quote_mint.key();
    reward_pool.quote_token_program = quote_token_program;
    reward_pool.acc_reward_per_weight = 0;
    reward_pool.reward_reserve = 0;
    reward_pool.protocol_fee_reserve = 0;
    reward_pool.last_reward_ts = 0;

    Ok(())
}
```

The only signer is `payer`. The instruction does not check that the launch
creator approves this quote mint.

---

## What a non-creator can do

1. **Front-run launch initialization.** As soon as a creator routes their Pump.fun
   bonding curve to the `fee_owner` PDA, any wallet can race to call
   `initialize_launch` first. The launch is created with the racer paying ~0.001 SOL
   of rent. The racer becomes the rent recipient if the account is ever closed
   — but no instruction closes a `launch`, so the rent is locked.
2. **Create `QuoteRewardPool` for arbitrary quote mints.** Any wallet can call
   `initialize_quote_rewards` with any SPL mint as `quote_mint`, paying the rent.
   The pool exists from that moment on; if `claim_*_quote_creator_fees` is
   subsequently called with that quote mint, fees flow into it.

Neither of these directly steals funds. They are griefing / squatting surfaces.

## What a non-creator cannot do

1. **Reroute creator fees.** The `fee_owner` PDA is deterministic from the mint;
   no other party controls fees attributable to that PDA. The bonding-curve
   `creator` field is owned by Pump.fun, not by this program.
2. **Bypass the route check.** `require_pump_creator_route` will reject any
   `initialize_launch` call where the bonding curve does not credit this
   program's `fee_owner` PDA either directly or via a Pump Fees `sharing_config`.
3. **Modify the launch after init.** There is no admin-style "update launch"
   instruction. The `launch` account's fields are write-once (apart from the
   reward accumulator and reserve).

---

## Threat scenarios

### Scenario A — Launch PDA rent griefing

An attacker monitors Pump.fun bonding-curve creator updates. The moment a
creator routes to a `fee_owner` PDA, the attacker calls `initialize_launch`. The
launch is now created and the attacker has paid the rent. The legitimate creator
proceeds as normal — they do not own the launch account anyway; the program does.
Impact: trivial griefing. No functional difference for the creator.

### Scenario B — Squatted `QuoteRewardPool`

An attacker creates a `QuoteRewardPool` for some random quote mint that the
creator has no intention of using. The pool sits inert. If the protocol later
adds a UI element listing all pools for a launch, the squatted pool appears.
Impact: UI noise. No funds.

### Scenario C — Unwanted quote-fee routing

If at some future point the protocol or community begins programmatically
claiming `claim_*_quote_creator_fees` for **every** existing `QuoteRewardPool`,
a previously-inert squatted pool would receive any fees routed to it. The fees
would be subject to MED-01 (locked if no enrolled stakers).

---

## Recommended actions

### Option A — Document the model

Add a "trust model" section to the README explaining that creators opt in via
Pump.fun routing, and that on-chain initialization of `launch` and
`QuoteRewardPool` PDAs is open. This is the lowest-friction option and
preserves the design.

### Option B — Gate `initialize_launch` behind the bonding-curve creator's
signature

Add a `creator: Signer` to the `InitializeLaunch` context and verify it matches
`pump_bonding_curve_creator(...)`. This eliminates Scenario A but requires the
creator to be present at init.

### Option C — Gate `initialize_quote_rewards` behind a per-launch authority

Store an `authority` field on `LaunchConfig` (set at init) and require it to
sign `initialize_quote_rewards`. This eliminates Scenario B/C. Requires the
HIGH-01 / MED fixes to compose cleanly.

### Option D — Leave as is, monitor

Permissionless creation is a feature of many DeFi protocols; document and move
on.

**Recommended:** **Option A** at minimum. Consider B or C if the team wants
tighter control over which launches and quote pools exist under this program.

---

## References

### Source files

| File | Relevance |
|------|-----------|
| `programs/staking/src/instructions/staking.rs` | `initialize_launch` |
| `programs/staking/src/instructions/rewards.rs` | `initialize_quote_rewards`, `initialize_quote_stake` |
| `programs/staking/src/guards.rs` | `require_pump_creator_route`, `require_pump_sharing_config` |
| `programs/staking/src/pda.rs` | `fee_owner_pda`, `pump_bonding_curve_pda`, `pump_sharing_config_pda` |

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial write-up. |
