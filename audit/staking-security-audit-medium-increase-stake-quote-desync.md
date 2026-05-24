# Staked Program — MED-02: `increase_stake` desynchronizes `QuoteStakeState` and under-pays the user

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
4. [Background](#background)
5. [Root cause](#root-cause)
6. [Vulnerable code paths](#vulnerable-code-paths)
7. [Step-by-step scenario](#step-by-step-scenario)
8. [Numerical example](#numerical-example)
9. [Comparison with HIGH-01](#comparison-with-high-01)
10. [What is NOT affected](#what-is-not-affected)
11. [On-chain detection](#on-chain-detection)
12. [Recommended fixes](#recommended-fixes)
13. [User guidance](#user-guidance)
14. [References](#references)

---

## Executive summary

`increase_stake` correctly settles **SOL** rewards (pays pending using the old
`stake_position.weight`, then rewrites `stake_position.reward_debt` with the new
weight). It does **not** perform the equivalent settlement against any
`QuoteStakeState` the user holds.

After `increase_stake`, `stake_position.weight` reflects the new (larger) weight
while `quote_stake.recorded_weight` still holds the **old** weight. Quote-token
rewards accruing between the increase and the user's next quote claim are credited
to the user at the **old** weight. When the user finally calls `claim_rewards`
(quote path), the handler pays based on the stale `recorded_weight`, then snaps
`recorded_weight` to the current `stake_position.weight` without back-paying the
new-vs-old delta.

The user is under-paid. The under-paid amount remains in
`QuoteRewardPool.reward_reserve` and is shared with future claimers (or stranded
per MED-01).

This is the inverse-direction foot-gun of HIGH-01: instead of stale weight being
**larger** than current (HIGH-01: exploitable), stale weight is **smaller** than
current (MED-02: user loss).

---

## Severity and impact

| Attribute | Assessment |
|-----------|------------|
| **Severity** | **Medium** |
| **Likelihood** | High once any user calls `increase_stake` with an active quote-pool enrolment. |
| **Exploit complexity** | N/A — this is silent user loss, not an attacker action. |
| **Funds at risk** | Quote rewards proportional to `(new_weight − old_weight)` times accrual between the increase and the next claim. |
| **Recovery path** | None — once `recorded_weight` is snapped on a subsequent claim, the under-payment is locked in. |

### Impact magnitude

For a user who doubles their stake via `increase_stake` and waits one quote reward
batch before claiming:

```text
under_payment ≈ batch × (new_weight − old_weight) / total_weighted_stake_at_batch
```

Across many users who `increase_stake` regularly without immediately claiming, the
aggregate stranded amount can be material.

---

## Affected scope

### Affected instructions

| Instruction | Role |
|-------------|------|
| `increase_stake` | Updates `StakePosition.weight` and SOL `reward_debt` but ignores `QuoteStakeState` |
| `claim_rewards` (quote path) | Pays using stale `quote_stake.recorded_weight`, then snaps to current `stake_position.weight` after payout |

### Not affected

- SOL rewards on `increase_stake` are paid correctly via
  `pending_reward(&stake_position, &launch)` before the weight bump.

---

## Background

`increase_stake` allows a staker to top up an existing `StakePosition` and/or extend
its lock. The handler:

1. Recomputes the weight from `(new_amount, lock_days)`.
2. Pays any pending SOL reward to the staker (read from `position.reward_debt` and
   `launch.acc_reward_per_weight`).
3. Updates `position.amount`, `position.weight`, `position.unlock_ts`,
   `position.reward_debt`.
4. Adjusts `launch.total_weighted_stake` by `(new_weight − old_weight)`.

The handler does **not** receive or modify any `QuoteStakeState` accounts. The
`IncreaseStake` accounts struct does not include them.

---

## Root cause

Two equivalent ways to state the bug:

**(a)** `increase_stake` mutates `stake_position.weight` without mutating the
linked `quote_stake.recorded_weight`, breaking the implicit invariant
`recorded_weight == stake_position.weight`.

**(b)** `claim_quote_rewards_from_remaining` snaps `recorded_weight` to
`stake_position.weight` **after** paying out, without back-crediting the
`(new_weight − old_weight)` delta against accrual that happened while the gap was
open.

---

## Vulnerable code paths

### 1. `IncreaseStake` accounts — no `QuoteStakeState`

```66:98:programs/staking/src/account_contexts.rs
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
    // ...
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
    // ...
}
```

### 2. `increase_stake` settles SOL but ignores quote state

```104:179:programs/staking/src/instructions/staking.rs
pub(crate) fn increase_stake(
    ctx: Context<IncreaseStake>,
    amount: u64,
    lock_days: u16,
) -> Result<()> {
    // ... validations ...

    let multiplier_bps = lock_multiplier_bps(lock_days)?;
    let unlock_ts = lock_unlock_ts(lock_days)?;
    let pending = pending_reward(&ctx.accounts.stake_position, &ctx.accounts.launch)?;
    lock_extension_allowed(ctx.accounts.stake_position.unlock_ts, unlock_ts)?;

    transfer_spl_tokens(/* ... */)?;

    let launch = &mut ctx.accounts.launch;
    if pending > 0 {
        pay_rewards(launch, &ctx.accounts.staker.to_account_info(), pending)?;
    }

    let position = &mut ctx.accounts.stake_position;
    let new_amount = position
        .amount
        .checked_add(amount)
        .ok_or(PobError::MathOverflow)?;
    let new_weight = weighted_amount(new_amount, multiplier_bps)?;

    launch.total_weighted_stake = launch
        .total_weighted_stake
        .checked_sub(position.weight)
        .ok_or(PobError::MathOverflow)?
        .checked_add(new_weight)
        .ok_or(PobError::MathOverflow)?;

    position.amount = new_amount;
    position.weight = new_weight;
    position.unlock_ts = unlock_ts;
    position.reward_debt = reward_debt(new_weight, launch.acc_reward_per_weight)?;

    Ok(())
}
```

The function never receives or touches a `QuoteStakeState`.

### 3. Quote claim snaps weight after payout

```157:178:programs/staking/src/instructions/rewards.rs
let mut quote_stake: Account<QuoteStakeState> = Account::try_from(quote_stake_info)?;
require_quote_stake(&quote_stake, &stake_position_key, &quote_reward_pool.key())?;

let pending = pending_quote_reward(&quote_stake, &quote_reward_pool)?;
if pending > 0 {
    pay_quote_rewards(/* ... */, pending, /* ... */)?;
}

quote_stake.recorded_weight = ctx.accounts.stake_position.weight;
quote_stake.reward_debt = reward_debt(
    ctx.accounts.stake_position.weight,
    quote_reward_pool.acc_reward_per_weight,
)?;
```

`pending` is computed from `quote_stake.recorded_weight` (old). The next two
assignments overwrite to `stake_position.weight` (new), so any delta accrued at
the new weight prior to this claim is lost.

---

## Step-by-step scenario

| Step | Action | `stake_position.weight` | `quote_stake.recorded_weight` | Effect |
|------|--------|--------------------------|-------------------------------|--------|
| 1 | `stake(100, 7d)` | 100 | (no quote_stake yet) | — |
| 2 | `initialize_quote_stake(usdc)` | 100 | 100 | enrolled |
| 3 | Quote batch B1 lands | 100 | 100 | acc grows; pending = 100·Δ |
| 4 | `increase_stake(+900, 30d)` | 1250 (1000 × 1.25×) | 100 | **DESYNC** |
| 5 | Quote batch B2 lands | 1250 | 100 | acc grows; pending = 100·Δ′ but user should get 1250·Δ′ |
| 6 | `claim_rewards` (quote path) | 1250 | 100 → 1250 | paid 100·(Δ+Δ′), should have been 100·Δ + 1250·Δ′ |
| 7 | After claim, `recorded_weight = 1250` | 1250 | 1250 | invariant restored, but B2 under-payment is locked in |

The under-paid amount remains in `QuoteRewardPool.reward_reserve`. The next
claimer's pending computation will be unaffected — their pending depends on their
own `recorded_weight` and the global `acc`. The orphaned slice mathematically
overlaps the MED-01 stranded-dust class.

---

## Numerical example

| Field | Value |
|-------|-------|
| Alice initial stake | 100,000 tokens × 1.00× = weight 100,000 |
| Alice `initialize_quote_stake` immediately | `recorded_weight = 100,000`, `reward_debt = 0` |
| `launch.total_weighted_stake` | 1,000,000 (Alice is 10%) |

**Batch B1 = 10 USDC arrives:**

```text
Δacc₁ = 10 × 1e18 / 1,000,000 = 1e13
pool.acc = 1e13
```

Alice's pending now: `100,000 × 1e13 / 1e18 = 1 USDC`. She does not claim.

**Alice calls `increase_stake(+400,000, 7d)`** (weight goes 100,000 → 500,000;
`total_weighted_stake` goes 1,000,000 → 1,400,000). Her `quote_stake.recorded_weight`
remains **100,000**.

**Batch B2 = 14 USDC arrives:**

```text
Δacc₂ = 14 × 1e18 / 1,400,000 = 1e13
pool.acc = 2e13
```

Alice's *true* fair share for B2 alone: `500,000 × 1e13 / 1e18 = 5 USDC`.
Alice's *true* fair share for B1: `1 USDC` (unchanged — she was at weight 100,000
when B1 arrived).
**Alice should receive: 6 USDC.**

**Alice claims:**

```text
pending = quote_stake.recorded_weight × pool.acc / 1e18 − quote_stake.reward_debt
        = 100,000 × 2e13 / 1e18 − 0
        = 2 USDC
```

Alice receives **2 USDC**. Then `recorded_weight = 500,000`,
`reward_debt = 500,000 × 2e13 / 1e18 = 10`.

**Under-payment: 4 USDC.** These 4 USDC remain in `reward_reserve` with no future
mechanism to deliver them to Alice.

---

## Comparison with HIGH-01

| Property | HIGH-01 | MED-02 |
|----------|---------|--------|
| Direction of stale weight | Recorded **>** current | Recorded **<** current |
| Trigger | `unstake` followed by re-stake | `increase_stake` |
| Affects | Honest stakers (theft) | The user themselves (loss) |
| Funds direction | Out of pool, to attacker | Stay in pool, stranded |
| Severity | High | Medium |

Both have the same structural cause: `QuoteStakeState` is not kept in lock-step
with `StakePosition.weight` outside of the quote-claim path itself. A single fix
that syncs `recorded_weight` on every weight-changing event addresses both.

---

## What is NOT affected

| Asset / flow | Reason |
|--------------|--------|
| SOL rewards on `increase_stake` | Settled via `pending_reward` before weight changes; uses `StakePosition` directly |
| Quote rewards for users who never call `increase_stake` | Invariant holds for them |
| Users who immediately call `claim_rewards` (quote) right after `increase_stake` | Synced before any new accrual; no loss |

---

## On-chain detection

For any wallet:

1. Read `stake_position.weight`.
2. For each `quote_stake` linked to that position, read `recorded_weight`.
3. If `recorded_weight < stake_position.weight`, the user has an open MED-02
   under-payment window. They should claim immediately to stop further drift.

A sustained gap across many users on the same launch is a strong signal that the
`increase_stake` flow is in active use without a sync.

---

## Recommended fixes

### Option A — Sync `QuoteStakeState` on `increase_stake` (preferred)

Extend `IncreaseStake` accounts with the user's `QuoteStakeState` accounts (or
iterate via remaining accounts). For each, before applying the weight change:

```rust
let pending = pending_quote_reward(quote_stake, reward_pool)?;
if pending > 0 {
    pay_quote_rewards(/* ... */, pending, /* ... */)?;
}
quote_stake.recorded_weight = new_weight;
quote_stake.reward_debt = reward_debt(new_weight, reward_pool.acc_reward_per_weight)?;
```

**Pros:** Eliminates the under-payment window; reuses existing quote-claim logic.
**Cons:** Requires extra accounts at call site; clients must enumerate the user's
quote stakes. A `sync_quote_stake` instruction can be added as a per-pool variant.

### Option B — Require sync before payout (paired with HIGH-01 fix)

Reject quote claims whose `recorded_weight != stake_position.weight`. Force the
user to call a sync instruction first. Same fix surface as the HIGH-01 mitigation
Option B.

**Pros:** One invariant check covers both HIGH-01 and MED-02.
**Cons:** Adds a required user step; UX impact unless the frontend auto-includes it.

### Option C — Lazy back-credit on claim

Modify `claim_quote_rewards_from_remaining` to track the pre-snap weight and
*back-credit* the delta. Requires storing per-snapshot history; complex and
state-heavy. Not recommended.

**Recommended:** **Option A** for new development, paired with the same
`recorded_weight == stake_position.weight` invariant enforced at claim time
(Option B).

---

## User guidance

- If you have an active `QuoteStakeState`, call `claim_rewards` (quote path)
  **immediately before** calling `increase_stake`. This zeroes the gap, so the
  weight bump does not strand any accrual.
- Alternatively, claim **immediately after** `increase_stake` to minimize the
  accrual window at the stale weight — but any batch that lands between the
  increase and the claim is still affected.
- Front-end integrators should auto-include a quote claim in the same transaction
  as `increase_stake` for every quote pool the user has enrolled in.

---

## References

### Source files

| File | Relevance |
|------|-----------|
| `programs/staking/src/instructions/staking.rs` | `increase_stake` handler |
| `programs/staking/src/account_contexts.rs` | `IncreaseStake` (no `QuoteStakeState`) |
| `programs/staking/src/instructions/rewards.rs` | `claim_quote_rewards_from_remaining` snap-after-pay logic |
| `programs/staking/src/rewards.rs` | `pending_quote_reward`, `reward_debt` |

### Formula reference

```text
under_payment(staker) = (new_weight − old_weight) × Σ Δacc_between_increase_and_claim / PRECISION
```

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial write-up. |
