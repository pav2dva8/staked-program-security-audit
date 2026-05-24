# Staked Program — Security Audit Notes

**Document version:** 1.0  
**Date:** 2026-05-24  
**Program:** `staked`  
**Program ID:** `aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK`  
**Repository:** [iceypump/staked](https://github.com/iceypump/staked)  
**Official audit status (per README):** Unaudited — not recommended for production without independent review.

This document records a **high-severity vulnerability** in quote-token reward accounting: a stale `QuoteStakeState` account survives `unstake`, allowing an attacker to claim quote rewards using inflated historical weight after re-staking a minimal amount.

---

## Table of contents

1. [Executive summary](#executive-summary)
2. [Severity and impact](#severity-and-impact)
3. [Affected scope](#affected-scope)
4. [Background: dual reward accounting](#background-dual-reward-accounting)
5. [Root cause](#root-cause)
6. [Vulnerable code paths](#vulnerable-code-paths)
7. [Missing validation](#missing-validation)
8. [Exploit prerequisites](#exploit-prerequisites)
9. [Step-by-step attack (narrative)](#step-by-step-attack-narrative)
10. [Numerical example](#numerical-example)
11. [What is NOT affected](#what-is-not-affected)
12. [On-chain detection](#on-chain-detection)
13. [Recommended fixes](#recommended-fixes)
14. [Operational response](#operational-response)
15. [User guidance](#user-guidance)
16. [References](#references)

---

## Executive summary

When a user calls `unstake`, the program closes their `StakePosition` PDA and correctly subtracts their weight from `LaunchConfig.total_weighted_stake`. However, **`unstake` does not close or reset any associated `QuoteStakeState` accounts**.

If the user previously called `initialize_quote_stake`, the quote account retains the old `recorded_weight` and `reward_debt`. After fully unstaking, the user can:

1. Wait while quote rewards accrue against the **lower** global stake (they are excluded from the divisor).
2. Re-stake a **tiny** amount (re-creating `StakePosition` at the same PDA address).
3. Call `claim_rewards` with quote remaining accounts.
4. Receive quote-token payouts computed from **stale `recorded_weight`**, not current `stake_position.weight`.

The attacker can drain `QuoteRewardPool.reward_reserve` and the quote reward vault ATA at the expense of honest stakers. SOL (native) rewards use `StakePosition` directly and are **not** subject to this specific bug.

---

## Severity and impact

| Attribute | Assessment |
|-----------|------------|
| **Severity** | **High** |
| **Likelihood** | High once `initialize_quote_stake` was used and quote fees flow |
| **Exploit complexity** | Low — no special privileges; any former staker with a live `quote_stake` account |
| **Funds at risk** | All quote tokens in `quote_reward_vault` / `reward_reserve` for affected pools |
| **Principal stake theft** | No — staked launch tokens follow normal unstake rules |
| **Reward theft** | Yes — disproportionate quote reward extraction |

### Impact on honest stakers

- Quote reward share is diluted or fully drained.
- Small stakers (e.g. &lt;0.01% pool share) suffer most relative to expected yield.
- Continued promotion of staking without fix or disclosure increases reputational and financial harm.

---

## Affected scope

### Affected instructions

| Instruction | Role in vulnerability |
|-------------|----------------------|
| `unstake` | Fails to close/sync `QuoteStakeState` |
| `claim_rewards` (quote path) | Pays using stale `recorded_weight` before syncing |
| `stake` | Allows re-init of `StakePosition` at same PDA, reusing old `quote_stake` |
| `initialize_quote_stake` | Creates the persistent quote snapshot (not buggy by itself) |

### Affected instructions (reward accrual — indirect)

Quote pools grow via fee claim paths that call `apply_quote_rewards` with `launch.total_weighted_stake`:

- `claim_pump_quote_creator_fees`
- `claim_pump_shared_quote_creator_fees`
- `claim_pumpswap_quote_creator_fees`
- `claim_pumpswap_shared_quote_creator_fees`

### Not affected

- SOL reward claims via `claim_rewards` with **no** remaining accounts (uses `stake_position.weight` live).
- `unstake` SOL reward settlement via `pending_reward(&stake_position, &launch)`.

---

## Background: dual reward accounting

The program maintains **two parallel weight snapshots** for each staker:

### 1. `StakePosition` (canonical live stake)

```rust
// programs/staking/src/state.rs
pub struct StakePosition {
    pub amount: u64,
    pub weight: u128,
    pub unlock_ts: i64,
    pub reward_debt: u128,  // for SOL rewards
}
```

Updated on: `stake`, `increase_stake`, `unstake` (closed), SOL `claim_rewards`.

### 2. `QuoteStakeState` (quote reward snapshot)

```rust
// programs/staking/src/state.rs
pub struct QuoteStakeState {
    pub stake_position: Pubkey,
    pub reward_pool: Pubkey,
    pub recorded_weight: u128,
    pub reward_debt: u128,
}
```

Created by: `initialize_quote_stake`  
Updated on: `claim_rewards` quote path **after** payout (too late for the current claim).

### PDA relationships

```text
stake_position = PDA(["stake", launch, owner])
quote_reward_pool = PDA(["quote-rewards", launch, quote_mint])
quote_stake = PDA(["quote-stake", stake_position, quote_reward_pool])
```

**Critical property:** After `unstake` closes `stake_position`, a subsequent `stake` re-initializes the **same** `stake_position` PDA (same seeds). The existing `quote_stake` PDA remains valid because it is keyed by `stake_position` pubkey, which is unchanged.

### Lock multipliers (weight calculation)

| Lock period | Multiplier | BPS |
|-------------|------------|-----|
| 7 days | 1.00× | 10,000 |
| 30 days | 1.25× | 12,500 |
| 90 days | 1.75× | 17,500 |
| 180 days | 2.50× | 25,000 |

```text
position_weight = staked_amount × multiplier_bps / 10_000
```

Constants: `ACC_REWARD_PRECISION = 1e18` (`programs/staking/src/constants.rs`).

---

## Root cause

**Invariant that should hold:**

```text
quote_stake.recorded_weight == stake_position.weight   (when stake_position exists and user is claiming)
```

**Actual behavior:**

1. `unstake` closes `stake_position` but leaves `quote_stake` on-chain unchanged.
2. `increase_stake` updates `stake_position.weight` but **never** touches `quote_stake`.
3. `claim_quote_rewards_from_remaining` computes pending rewards from `quote_stake.recorded_weight`, not `stake_position.weight`.
4. Weight sync on claim happens **after** `pay_quote_rewards`.

---

## Vulnerable code paths

### 1. `Unstake` account struct — no `quote_stake`

```192:201:programs/staking/src/account_contexts.rs
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
```

No `QuoteStakeState` account is passed; nothing closes or zeroes quote state.

### 2. `unstake` handler — global weight updated, quote state ignored

```213:230:programs/staking/src/instructions/staking.rs
    launch.total_weighted_stake = launch
        .total_weighted_stake
        .checked_sub(position.weight)
        .ok_or(PobError::MathOverflow)?;

  // ... transfers staked tokens back to user ...

    Ok(())
```

### 3. Quote pending reward uses `recorded_weight`

```166:178:programs/staking/src/rewards.rs
pub(crate) fn pending_quote_reward(
    quote_stake: &QuoteStakeState,
    reward_pool: &QuoteRewardPool,
) -> Result<u64> {
    let accrued = reward_debt(
        quote_stake.recorded_weight,
        reward_pool.acc_reward_per_weight,
    )?;
    let pending = accrued
        .checked_sub(quote_stake.reward_debt)
        .ok_or(PobError::MathOverflow)?;
```

### 4. Claim pays first, syncs second

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

### 5. Quote reward accrual uses current global stake (attacker excluded after unstake)

```138:156:programs/staking/src/rewards.rs
pub(crate) fn apply_quote_rewards(
    reward_pool: &mut QuoteRewardPool,
    total_weighted_stake: u128,
    amount: u64,
) -> Result<()> {
    // ...
    let increment = (amount as u128)
        .checked_mul(ACC_REWARD_PRECISION)
        .ok_or(PobError::MathOverflow)?
        .checked_div(total_weighted_stake)
        .ok_or(PobError::MathOverflow)?;
```

After unstake, `total_weighted_stake` drops, so `acc_reward_per_weight` rises faster per unit of **remaining** stake — while the attacker's stale `quote_stake` still credits them as if they retained old weight for the entire accrual period.

---

## Missing validation

`require_quote_stake` only checks pubkey linkage:

```78:93:programs/staking/src/guards.rs
pub(crate) fn require_quote_stake(
    quote_stake: &QuoteStakeState,
    stake_position: &Pubkey,
    reward_pool: &Pubkey,
) -> Result<()> {
    require_keys_eq!(quote_stake.stake_position, *stake_position, /* ... */);
    require_keys_eq!(quote_stake.reward_pool, *reward_pool, /* ... */);
    Ok(())
}
```

**Missing check:**

```rust
require!(
    quote_stake.recorded_weight == stake_position.weight,
    PobError::QuoteStakeOutOfSync  // hypothetical error
);
```

---

## Exploit prerequisites

1. Attacker previously staked a **large** amount and called `initialize_quote_stake` for at least one quote pool.
2. Attacker **fully unstaked** (or could have partially reduced stake — any mismatch between live weight and `recorded_weight` is exploitable; full unstake is the clearest case).
3. Attacker's `quote_stake` account was **not** closed.
4. Quote rewards continued to accrue (`claim_pump_quote_creator_fees` or related paths).
5. Attacker **re-staked** a minimal amount before claiming quote rewards.
6. Sufficient balance in `quote_reward_pool.reward_reserve` / vault.

**Permissions required:** None beyond normal user signing. No admin or upgrade authority needed.

---

## Step-by-step attack (narrative)

| Step | Action | On-chain state |
|------|--------|----------------|
| 1 | `stake(large_amount, lock_days)` | `stake_position.weight = W_large` |
| 2 | `initialize_quote_stake` | `quote_stake.recorded_weight = W_large`, `reward_debt = 0` |
| 3 | (Optional) Quote fees accrue while staked | Fair rewards accumulate |
| 4 | Wait until `unlock_ts`, then `unstake` | `stake_position` **closed**; `total_weighted_stake -= W_large`; **`quote_stake` unchanged** |
| 5 | Quote fees accrue (attacker out of pool) | `acc_reward_per_weight` increases; divisor excludes attacker |
| 6 | `stake(1, lock_days)` | New `stake_position` at **same PDA**; `weight = W_tiny` (e.g. 1) |
| 7 | `claim_rewards` + 6 quote remaining accounts | Payout ≈ `f(W_large, acc)` — **not** `f(W_tiny, acc)` |
| 8 | Repeat steps 5–7 as new fees arrive | Can drain vault until reserve exhausted |

---

## Numerical example

**Assumptions:** 7-day lock (1.00×), so weight = token amount. Three stakers, Alice/Bob/Carol each 100,000 tokens.

### Initial state

| Field | Value |
|-------|-------|
| `total_weighted_stake` | 300,000 |
| Alice `stake_position.weight` | 100,000 |
| Alice `quote_stake.recorded_weight` | 100,000 |
| Alice `quote_stake.reward_debt` | 0 |
| `acc_reward_per_weight` | 0 |

### Round 1 — 10 quote tokens deposited (all staked)

```text
Δacc = 10 × 1e18 / 300,000 ≈ 3.33 × 10^13
```

Fair share per staker: **3.33 tokens** each. Alice does not claim.

### Alice unstakes

| Field | Before | After |
|-------|--------|-------|
| `total_weighted_stake` | 300,000 | **200,000** |
| Alice `stake_position` | exists | **closed** |
| Alice `quote_stake.recorded_weight` | 100,000 | **100,000 (stale)** |

### Round 2 — 10 quote tokens deposited (Alice out)

```text
Δacc = 10 × 1e18 / 200,000 = 5.0 × 10^13
Total acc ≈ 8.33 × 10^13
```

Fair for Alice this round: **0**. Bob and Carol: **5.0** each.

### Alice re-stakes 1 token and claims

| Field | Value |
|-------|-------|
| `stake_position.weight` | **1** |
| `quote_stake.recorded_weight` used in payout | **100,000** |

```text
accrued = 100,000 × 8.33e13 / 1e18 ≈ 8.33 tokens
pending = 8.33 - 0 = 8.33 tokens
```

| | Tokens |
|--|--------|
| **Fair total for Alice** | 3.33 (round 1 only) |
| **Actual claim** | **8.33** |
| **Stolen from Bob/Carol** | **~5.0** |

Alice's live stake during the claim: **1 token**.

### Scale example (large pool)

Pool ~127M tokens staked. Attacker with former weight **12,500,000** (10M tokens × 1.25×) unstakes, then re-stakes **1 token**. One quote distribution of **100 WSOL** while active weight is **50M**:

- Honest share if still at 12.5M weight: ~25 WSOL for that event (if synced).
- With stale weight on accumulated `acc` since last claim: can extract rewards as if still a whale while only 1 token is staked.

---

## What is NOT affected

| Asset / flow | Reason |
|--------------|--------|
| Staked launch tokens (principal) | Returned on `unstake` via SPL transfer; not inflated by this bug |
| SOL rewards | `pending_reward` reads `stake_position.weight` directly |
| Protocol fee vaults | Separate accounting |
| Users who never called `initialize_quote_stake` | No `quote_stake` account to abuse |

---

## On-chain detection

### Program to inspect

`aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK`

### Suspicious transaction pattern

1. Historical: large `stake` + `initialize_quote_stake`
2. `unstake` — `StakePosition` account closed in transaction accounts
3. Time gap with quote fee claim transactions increasing pool `acc_reward_per_weight`
4. Small `stake` (minimal amount)
5. `claim_rewards` with 6 remaining accounts — large SPL transfer from `quote_reward_vault_ata` to user quote ATA

### Accounts to compare

For a wallet `owner` and launch mint:

1. Derive `launch = PDA(["launch", mint])`
2. Derive `stake_position = PDA(["stake", launch, owner])`
3. Fetch on-chain `StakePosition` → read `weight`
4. For each quote mint, derive `quote_reward_pool` and `quote_stake`
5. Fetch `QuoteStakeState` → read `recorded_weight`

**Red flag:** `recorded_weight >> stake_position.weight` (or stake exists with tiny weight while quote_stake shows historical whale weight).

### Solscan / explorer fields

- Instruction: `claim_rewards` on program `aZPKJ...BANK`
- Inner instruction: SPL token transfer from quote reward vault
- Compare token amount received vs user's current staked amount and pool share

---

## Recommended fixes

### Option A — Close `quote_stake` on unstake (preferred)

Extend `Unstake` accounts to include every associated `QuoteStakeState` (or iterate remaining accounts) and close them to `owner`, returning rent.

**Pros:** Stale state cannot survive; user must call `initialize_quote_stake` again after re-staking.  
**Cons:** Must pass quote stake accounts at unstake (or add a dedicated `close_quote_stake` instruction).

### Option B — Enforce sync before payout

In `claim_quote_rewards_from_remaining`, before `pending_quote_reward`:

```rust
require!(
    quote_stake.recorded_weight == ctx.accounts.stake_position.weight,
    PobError::QuoteStakeOutOfSync
);
```

**Pros:** Minimal change; blocks exploit immediately.  
**Cons:** Users must call a sync path after `increase_stake` / weight changes or claim fails until they do.

### Option C — Sync on stake lifecycle events

Update or reset `quote_stake.recorded_weight` in `stake`, `increase_stake`, and `unstake` (or close on unstake).

### Option D — Operational (not a code substitute)

- Pause quote claims via frontend / coordination
- Upgrade program with fix
- Publish disclosure

**Recommended:** Combine **Option A** (close on unstake) **and** **Option B** (defense in depth on claim).

---

## Operational response

If the team is aware and production staking is live, a reasonable response includes:

1. **Disclose** the issue publicly to stakers.
2. **Pause** new staking and quote claims until patched (or warn prominently in UI).
3. **Deploy** fixed program or upgrade (with verifier hash published).
4. **Monitor** quote vault outflows for exploit pattern above.
5. **Independent audit** before re-promoting staking.

Continuing to promote staking without fix or warning exposes users to reward drainage and creates severe trust/reputational risk.

---

## User guidance

### If you are staked now

| Risk | Detail |
|------|--------|
| Principal | Still subject to lock and normal program rules |
| Quote rewards | May be drained by abusers; small stakers lose expected yield |
| Adding stake | Increases exposure; does not fix quote sync for others |

### Practical steps

1. **Do not add** to position until fix is verified on-chain.
2. **After unlock**, consider unstaking if no fix is deployed.
3. **Document** UI state, transactions, and any team communications.
4. **Inspect** quote vault and large `claim_rewards` transfers on Solscan for your launch mint.

### Is this a "scam"?

This is a **code vulnerability**, not necessarily a fake token. It becomes **predatory** if insiders know of the bug, continue promoting staking, or exploit it themselves while others deposit. The README already states the program is **unaudited**.

---

## References

### Source files

| File | Relevance |
|------|-----------|
| `programs/staking/src/account_contexts.rs` | `Unstake`, `InitializeQuoteStake`, `ClaimRewards` account layouts |
| `programs/staking/src/instructions/staking.rs` | `unstake`, `stake`, `increase_stake` |
| `programs/staking/src/instructions/rewards.rs` | `claim_quote_rewards_from_remaining`, `initialize_quote_stake` |
| `programs/staking/src/rewards.rs` | `apply_quote_rewards`, `pending_quote_reward`, `reward_debt` |
| `programs/staking/src/guards.rs` | `require_quote_stake` |
| `programs/staking/src/state.rs` | Account struct definitions |
| `programs/staking/src/constants.rs` | `ACC_REWARD_PRECISION`, lock BPS |
| `programs/staking/README.md` | Architecture, PDA seeds, instruction list |

### External

- Program ID: `aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK`
- Security policy: https://github.com/iceypump/staked/security/policy
- Example deploy transaction (program upload, **not** an exploit tx): https://solscan.io/tx/vVahiDVgJ5oxH3KrKiwPF4kiwhVUzRAsauuRxevKRju8Ex3ADoEQfjJcXtViMG92LvpSUrGvwC1bx6XfU1V3eJk

### Reward formula reference

```text
reward_debt(weight, acc) = weight × acc / ACC_REWARD_PRECISION

pending_quote = reward_debt(quote_stake.recorded_weight, pool.acc)
                - quote_stake.reward_debt

Δacc on deposit = amount × ACC_REWARD_PRECISION / total_weighted_stake
```

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial write-up: stale `quote_stake` after `unstake` |
