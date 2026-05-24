# Staked Program — MED-01: Quote rewards permanently lock when stakers never initialize `QuoteStakeState`

**Document version:** 1.0
**Date:** 2026-05-24
**Program:** `staked`
**Program ID:** `aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK`
**Severity:** **Medium**
**Status:** Unfixed at the time of writing.

---

## Table of contents

1. [Executive summary](#executive-summary)
2. [Severity and impact](#severity-and-impact)
3. [Affected scope](#affected-scope)
4. [Background: dual accounting for quote rewards](#background-dual-accounting-for-quote-rewards)
5. [Root cause](#root-cause)
6. [Vulnerable code paths](#vulnerable-code-paths)
7. [Step-by-step scenario](#step-by-step-scenario)
8. [Numerical example](#numerical-example)
9. [Conditions under which the lock occurs](#conditions-under-which-the-lock-occurs)
10. [What is NOT affected](#what-is-not-affected)
11. [On-chain detection](#on-chain-detection)
12. [Recommended fixes](#recommended-fixes)
13. [User guidance](#user-guidance)
14. [References](#references)

---

## Executive summary

`apply_quote_rewards` distributes incoming quote-token reward batches using
`launch.total_weighted_stake` as the divisor. That divisor counts **every** active
`StakePosition`, regardless of whether each staker has called `initialize_quote_stake`
for the relevant `QuoteRewardPool`.

A staker who has not initialized a `QuoteStakeState` for a pool **dilutes the per-weight
accrual** (`acc_reward_per_weight`) without being able to claim. When such a staker
belatedly initializes their quote stake, `initialize_quote_stake` snapshots
`reward_debt` at the current `acc_reward_per_weight`, locking them out of the share
they previously diluted.

The result: a portion of the quote-reward batch is **permanently stranded** in
`QuoteRewardPool.reward_reserve` (and the on-chain quote reward vault ATA). No
instruction allows the protocol to recover it. Honest claimers can never claim the
"unowned" slice because the program enforces `recorded_weight × Δacc − reward_debt`
for each enrolled staker, and the un-enrolled staker's slice has no owner who can
reach it.

---

## Severity and impact

| Attribute | Assessment |
|-----------|------------|
| **Severity** | **Medium** |
| **Likelihood** | High — the UI does not enforce or hide `initialize_quote_stake`, and the instruction is permissionless and independent of `stake`. Many users will stake without enrolling in every quote pool. |
| **Exploit complexity** | None — this is a stranding bug, not an attacker action. |
| **Funds at risk** | Quote rewards proportional to un-enrolled stakers' weight share, per quote pool. |
| **Recovery path** | None on-chain. Requires program upgrade to add a sweep instruction or to gate enrolment behind staking. |

### Impact magnitude

The stranded amount per reward batch is approximately:

```text
stranded = batch × (Σ un-enrolled weight) / total_weighted_stake
```

If half of staker weight has not initialized the quote pool, **half** of every quote
reward batch is permanently stranded for that pool.

---

## Affected scope

### Affected instructions

| Instruction | Role |
|-------------|------|
| `apply_quote_rewards` (helper) | Divides by `total_weighted_stake` including un-enrolled stakers |
| `initialize_quote_stake` | Sets `reward_debt` from current `acc_reward_per_weight`, locking the user out of past rewards |
| `claim_pump_quote_creator_fees` | Calls `apply_quote_rewards_with_protocol_fee` |
| `claim_pump_shared_quote_creator_fees` | Same |
| `claim_pumpswap_quote_creator_fees` | Same |
| `claim_pumpswap_shared_quote_creator_fees` | Same |

### Not affected

- SOL reward accounting via `acc_reward_per_weight` on `LaunchConfig`: there is no
  separate enrolment step. Every staker automatically participates by virtue of
  having a `StakePosition`. `claim_rewards` (no remaining accounts) reads
  `stake_position` directly.

---

## Background: dual accounting for quote rewards

Quote-token rewards are tracked **per `(launch, quote_mint)` pair** by a separate
`QuoteRewardPool` PDA. Each staker who wishes to participate must additionally hold
a `QuoteStakeState` PDA linking their `StakePosition` to the `QuoteRewardPool`.

```text
launch              = PDA(["launch", mint])
quote_reward_pool   = PDA(["quote-rewards", launch, quote_mint])
stake_position      = PDA(["stake", launch, owner])
quote_stake         = PDA(["quote-stake", stake_position, quote_reward_pool])
```

The two enrolment steps are independent:

1. `stake(amount, lock_days)` — creates `StakePosition`. Required.
2. `initialize_quote_stake(...)` — creates `QuoteStakeState` for a specific quote
   pool. **Optional; one call per quote pool.**

There is no enforcement that `(2)` happens before quote rewards begin to flow, nor
any mechanism for `(2)` to back-credit past accrual.

---

## Root cause

`apply_quote_rewards` ([programs/staking/src/rewards.rs:138-164](../programs/staking/src/rewards.rs#L138)) uses
`total_weighted_stake` — the **global** stake counter on `LaunchConfig` — as the
divisor. That counter is the sum of every `StakePosition.weight`, whether or not the
owner has an associated `QuoteStakeState`.

```text
Δacc = amount × ACC_REWARD_PRECISION / total_weighted_stake
acc_reward_per_weight += Δacc
reward_reserve += amount
```

Each enrolled staker can later claim `recorded_weight × Δacc / ACC_REWARD_PRECISION`.
Sum over enrolled stakers is strictly less than `amount` whenever **any** un-enrolled
weight is present in `total_weighted_stake`. The difference accumulates in
`reward_reserve` (and the corresponding token ATA balance) with no claim path.

`initialize_quote_stake` ([programs/staking/src/instructions/rewards.rs:55-73](../programs/staking/src/instructions/rewards.rs#L55))
then snapshots the **current** `acc_reward_per_weight` into `reward_debt`:

```rust
quote_stake.recorded_weight = stake_position.weight;
quote_stake.reward_debt = reward_debt(
    stake_position.weight,
    quote_reward_pool.acc_reward_per_weight,
)?;
```

So a late-enrolling user's pending starts at zero from the moment of enrolment.
Their share of past accrual is permanently inaccessible.

---

## Vulnerable code paths

### 1. Reward accrual uses global stake

```138:164:programs/staking/src/rewards.rs
pub(crate) fn apply_quote_rewards(
    reward_pool: &mut QuoteRewardPool,
    total_weighted_stake: u128,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, PobError::NoRewards);
    require!(total_weighted_stake > 0, PobError::NoActiveStake);

    let increment = (amount as u128)
        .checked_mul(ACC_REWARD_PRECISION)
        .ok_or(PobError::MathOverflow)?
        .checked_div(total_weighted_stake)
        .ok_or(PobError::MathOverflow)?;
    require!(increment > 0, PobError::NoRewards);

    reward_pool.acc_reward_per_weight = reward_pool
        .acc_reward_per_weight
        .checked_add(increment)
        .ok_or(PobError::MathOverflow)?;
    reward_pool.reward_reserve = reward_pool
        .reward_reserve
        .checked_add(amount)
        .ok_or(PobError::MathOverflow)?;
    reward_pool.last_reward_ts = Clock::get()?.unix_timestamp;

    Ok(())
}
```

Callers pass `launch.total_weighted_stake`:

```207:211:programs/staking/src/instructions/fees.rs
        ctx.accounts.launch.total_weighted_stake,
        ctx.bumps.fee_owner,
        collected,
    )?;
```

### 2. Late enrolment snapshots current accumulator

```55:73:programs/staking/src/instructions/rewards.rs
pub(crate) fn initialize_quote_stake(ctx: Context<InitializeQuoteStake>) -> Result<()> {
    require_quote_reward_pool(
        &ctx.accounts.quote_reward_pool,
        &ctx.accounts.launch.key(),
        &ctx.accounts.quote_reward_pool.quote_mint,
        &ctx.accounts.quote_reward_pool.quote_token_program,
    )?;

    let quote_stake = &mut ctx.accounts.quote_stake;
    quote_stake.stake_position = ctx.accounts.stake_position.key();
    quote_stake.reward_pool = ctx.accounts.quote_reward_pool.key();
    quote_stake.recorded_weight = ctx.accounts.stake_position.weight;
    quote_stake.reward_debt = reward_debt(
        ctx.accounts.stake_position.weight,
        ctx.accounts.quote_reward_pool.acc_reward_per_weight,
    )?;

    Ok(())
}
```

### 3. No protocol-side sweep of unclaimed quote dust

`claim_quote_protocol_fees` sweeps only the `quote_protocol_fee_vault_ata`, not the
main `quote_reward_vault_ata`. There is no instruction that can move stranded
reward-reserve dust back to a treasury or redistribute it.

---

## Step-by-step scenario

| Step | Action | Effect on quote pool |
|------|--------|----------------------|
| 1 | Alice `stake(100, 7d)` and `initialize_quote_stake(usdc_pool)` | `total_weighted_stake = 100`; Alice enrolled |
| 2 | Bob `stake(100, 7d)` — does **not** call `initialize_quote_stake` | `total_weighted_stake = 200`; only Alice enrolled |
| 3 | `claim_pump_quote_creator_fees` lands 10 USDC | `acc += 10·1e18 / 200`; `reward_reserve = 10` |
| 4 | Alice claims | Alice pending = `100 × Δacc / 1e18 = 5 USDC`; reserve = 5 |
| 5 | Bob calls `initialize_quote_stake(usdc_pool)` | Bob's `reward_debt = 100 × current_acc / 1e18`; Bob pending = 0 |
| 6 | Bob attempts to claim | pending = 0 — nothing to receive |
| 7 | Reserve stays at 5 USDC, with no claim path | **Stranded** |

The 5 USDC remains in `reward_reserve` and the corresponding ATA. No instruction
can disburse it without a program upgrade.

---

## Numerical example

Three stakers, equal 100,000-token positions at 7-day lock (multiplier 1.00×):

| Staker | `StakePosition.weight` | Enrolled in USDC pool? |
|--------|------------------------|------------------------|
| A | 100,000 | Yes (`initialize_quote_stake` called) |
| B | 100,000 | No |
| C | 100,000 | No |

`launch.total_weighted_stake = 300,000`.

A USDC reward batch of **30 USDC** arrives:

```text
Δacc = 30 × 1e18 / 300,000 = 1e14
acc_reward_per_weight = 1e14
reward_reserve = 30 USDC
```

Claims per enrolled staker:

```text
A: pending = 100,000 × 1e14 / 1e18 = 10 USDC
B: not enrolled
C: not enrolled
```

A claims 10 USDC. Reserve becomes 20 USDC.

B and C later call `initialize_quote_stake`. Each snapshots
`reward_debt = 100,000 × 1e14 / 1e18 = 10`. Their pending from this point forward
starts at zero relative to the **current** `acc_reward_per_weight`.

**20 USDC** sits in `QuoteRewardPool.reward_reserve` forever. The next reward batch
will be divided across all three (now enrolled), but the 20 USDC carryover has no
claim mechanism.

---

## Conditions under which the lock occurs

1. At least one active `StakePosition` exists whose owner has not initialized a
   `QuoteStakeState` for the relevant `QuoteRewardPool`.
2. Quote rewards are claimed into the pool via any of the four
   `claim_*_quote_creator_fees` paths.
3. The un-enrolled staker either never enrols, or enrols after rewards have
   accrued.

In a typical mainnet flow where users stake via a frontend that does not auto-call
`initialize_quote_stake` for every existing quote pool, condition (1) is the default
state for new stakers.

---

## What is NOT affected

| Asset / flow | Reason |
|--------------|--------|
| Staked launch tokens (principal) | Returned on `unstake` independent of quote accounting |
| SOL rewards | No enrolment step; every `StakePosition` participates automatically |
| Already-claimed quote rewards | Once paid out, not at risk |
| Protocol fee splits | `apply_quote_rewards_with_protocol_fee` removes the protocol fee before division |

---

## On-chain detection

For a given `QuoteRewardPool`:

1. Read `reward_reserve`.
2. Read the token balance of the `quote_reward_vault_ata`.
3. Compute the sum of `pending_quote_reward(quote_stake, pool)` for every
   `QuoteStakeState` that links to this pool.
4. `pool_balance − Σ pending` is the **stranded** amount (modulo rounding dust from
   integer division of `Δacc`).

This computation requires enumerating `QuoteStakeState` accounts by
`reward_pool == pool.key()` (use `getProgramAccounts` with an offset filter on the
discriminator + the 32-byte `reward_pool` field).

A non-trivial gap between `reward_reserve` and `Σ pending` over time indicates the
lock-up is active.

---

## Recommended fixes

### Option A — Auto-enrol on `stake` (preferred for new launches)

Extend the `Stake` accounts struct with an optional list of `QuoteStakeState`
accounts to initialize at the same time, **OR** track a per-launch list of active
quote pools and require initialization of all of them on `stake`. This is
client-heavy; a cleaner variant is a single `enroll_all_quote_pools` instruction
called after `stake`.

### Option B — Track enrolled weight separately (preferred for existing launches)

Add a second counter on `QuoteRewardPool` for enrolled weight:

```rust
pub struct QuoteRewardPool {
    // ...existing fields...
    pub enrolled_weighted_stake: u128,
}
```

Update on `initialize_quote_stake`, on `claim_rewards` quote path (already syncs
weight), and on `unstake` (subtract). Use this counter as the divisor in
`apply_quote_rewards`.

**Pros:** Conservation holds — every accrued unit has an owner.
**Cons:** Requires synchronization of `enrolled_weighted_stake` on every
weight-changing event, including the unstake fix from HIGH-01.

### Option C — Protocol sweep of stranded dust

Add a privileged instruction that, after computing
`pool_balance − Σ pending`, moves the gap to a treasury ATA. Requires off-chain
indexing to validate the gap; not ideal because correctness depends on indexer
honesty.

### Option D — Documentation only (not recommended)

Disclose the requirement in the README and have the frontend always call
`initialize_quote_stake` immediately after `stake` for every active pool. Does not
fix the underlying invariant violation and depends on every integrator behaving
correctly.

**Recommended:** **Option B**, combined with the unstake fix from HIGH-01 so
`enrolled_weighted_stake` is correctly decremented when a `QuoteStakeState` is
closed.

---

## User guidance

- Call `initialize_quote_stake` for **every** quote pool of interest **before** any
  quote-fee claim transaction lands on the pool. Otherwise your share of that batch
  is unrecoverable.
- Front-end integrators should auto-detect existing `QuoteRewardPool` PDAs for a
  launch and prompt enrolment in the same transaction as `stake`.

---

## References

### Source files

| File | Relevance |
|------|-----------|
| `programs/staking/src/rewards.rs` | `apply_quote_rewards`, `reward_debt` |
| `programs/staking/src/instructions/rewards.rs` | `initialize_quote_stake`, `claim_quote_rewards_from_remaining` |
| `programs/staking/src/instructions/fees.rs` | All four `claim_*_quote_creator_fees` callers |
| `programs/staking/src/state.rs` | `QuoteRewardPool`, `QuoteStakeState` |

### Reward formula reference

```text
Δacc = batch × ACC_REWARD_PRECISION / launch.total_weighted_stake
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
       includes un-enrolled stakers; "their" share has no owner

pending_quote(staker) = quote_stake.recorded_weight × Δacc / PRECISION
                        − quote_stake.reward_debt
```

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial write-up. |
