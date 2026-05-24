# Staked Program — INFO-01: First staker can claim 100% of any creator-fee backlog accumulated before staking began

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
4. [Background](#background)
5. [Mechanism](#mechanism)
6. [Vulnerable / behavioural code paths](#vulnerable--behavioural-code-paths)
7. [Numerical example](#numerical-example)
8. [Trust model implications](#trust-model-implications)
9. [Recommended actions](#recommended-actions)
10. [References](#references)

---

## Executive summary

`stake` skips the "clean creator-vault" precondition when
`launch.total_weighted_stake == 0`. This means a launch can be initialized and
left without any staker while Pump.fun creator fees accumulate in the
`creator_vault` (direct or shared-config route). When the first staker arrives,
they deposit any amount with any valid lock period, and then the **next**
`claim_pump_*creator_fees` call applies the entire backlog to
`acc_reward_per_weight` divided by the first staker's solitary weight. They
collect approximately 100% of the backlog on their first claim.

This is the intended behaviour: the program rewards bootstrapping the pool with
the first lock. But it concentrates a potentially large historical backlog into
a single participant's hands. Worth documenting and confirming with the team
that the trade-off is desired.

---

## Severity and intent

| Attribute | Assessment |
|-----------|------------|
| **Severity** | Informational |
| **Intent** | Likely deliberate — first-mover incentive |
| **Funds at risk** | None directly. Distribution skew, not theft. |
| **Concern** | If insiders pre-route fees to a launch's `fee_owner` PDA for some time before announcing staking, the first staker (potentially an insider) collects the backlog. |

---

## Affected scope

### Affected instructions

| Instruction | Role |
|-------------|------|
| `initialize_launch` | Sets up the `launch` and `fee_owner` PDA. Fees begin accruing in the Pump.fun `creator_vault` after this point if the bonding-curve creator has been routed. |
| `stake` (first invocation) | Skips the clean-vault check when `total_weighted_stake == 0` |
| `claim_pump_creator_fees`, `claim_pump_shared_creator_fees` | Sweep accumulated backlog into rewards, distributed by `total_weighted_stake = first_staker.weight` |

---

## Background

The program uses a "must drain before joining" pattern to prevent reward dilution
by new entrants:

```62:73:programs/staking/src/instructions/staking.rs
if ctx.accounts.launch.total_weighted_stake > 0 {
    require_clean_creator_vault_for_route(
        &ctx.accounts.creator_vault.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.remaining_accounts,
    )?;
    require_clean_pumpswap_creator_vault_for_route(
        &ctx.accounts.coin_creator_vault_ata.to_account_info(),
        &ctx.accounts.launch.mint,
        ctx.remaining_accounts,
    )?;
}
```

The guard ensures that an existing staker's accrued share is realized before any
new staker dilutes the divisor. When `total_weighted_stake == 0`, there are no
existing stakers to protect, so the check is skipped. This is sound.

What it does **not** do is accrue the pre-existing creator-vault balance to a
treasury or burn it. The first staker becomes the implicit owner of the entire
backlog.

---

## Mechanism

The sequence is:

1. `initialize_launch` runs. The bonding-curve's `creator` field is set to the
   program's `fee_owner` PDA (or to a sharing config that includes the PDA).
   Pump.fun begins forwarding creator fees to the program-controlled
   `creator_vault`.
2. Time passes. Trading volume accumulates fees in the `creator_vault`.
3. **Optionally:** the team publicly announces staking is open.
4. The first staker calls `stake(amount, lock_days)`. The clean-vault check is
   skipped because `total_weighted_stake == 0`.
5. Now `total_weighted_stake = first_staker.weight`.
6. Any actor calls `claim_pump_creator_fees` (or the shared variant). The
   handler:
   - CPIs into Pump.fun's `CollectCreatorFee` instruction. This empties the
     `creator_vault` into the `fee_owner` PDA.
   - Sweeps `fee_owner.lamports() − rent_floor` into `launch`.
   - Calls `apply_rewards_with_protocol_fee` — which divides the sweep amount by
     `total_weighted_stake = first_staker.weight`, giving the first staker
     ≈100% of `acc_reward_per_weight` growth attributable to the backlog.
7. The first staker claims their share via `claim_rewards`. They receive the
   entire backlog (minus the 25% protocol fee).

---

## Vulnerable / behavioural code paths

### 1. Clean-vault skip in `stake`

```62:73:programs/staking/src/instructions/staking.rs
if ctx.accounts.launch.total_weighted_stake > 0 {
    require_clean_creator_vault_for_route(/* ... */)?;
    require_clean_pumpswap_creator_vault_for_route(/* ... */)?;
}
```

### 2. Reward accrual divisor is current `total_weighted_stake`

```46:66:programs/staking/src/rewards.rs
pub(crate) fn apply_rewards(launch: &mut LaunchConfig, amount: u64) -> Result<()> {
    require!(launch.total_weighted_stake > 0, PobError::NoActiveStake);

    let increment = (amount as u128)
        .checked_mul(ACC_REWARD_PRECISION)
        .ok_or(PobError::MathOverflow)?
        .checked_div(launch.total_weighted_stake)
        .ok_or(PobError::MathOverflow)?;
    require!(increment > 0, PobError::NoRewards);

    launch.acc_reward_per_weight = launch
        .acc_reward_per_weight
        .checked_add(increment)
        .ok_or(PobError::MathOverflow)?;
    launch.reward_reserve = launch
        .reward_reserve
        .checked_add(amount)
        .ok_or(PobError::MathOverflow)?;

    Ok(())
}
```

### 3. No "burn-or-treasury-on-init" mechanism

`initialize_launch` does not record the creator-vault balance at init nor sweep
any pre-existing balance to a neutral destination. Whatever is in the vault when
the first staker arrives is attributed proportionally to weight (which, by
construction, is entirely the first staker).

---

## Numerical example

Assume the launch has accumulated **100 SOL** of creator fees in the Pump.fun
`creator_vault` between `initialize_launch` and the first `stake` call.

| Step | Action | State |
|------|--------|-------|
| 0 | `initialize_launch` | `total_weighted_stake = 0`, `acc = 0`, `creator_vault = 0` |
| 1 | Trading volume over 24h | `creator_vault = 100 SOL` |
| 2 | Alice `stake(1 token, 7d)` | `total_weighted_stake = 1` (raw weight 1×10⁶ at 6 decimals, multiplier 1.0×) |
| 3 | Any caller `claim_pump_creator_fees` | sweep ≈ 100 SOL into `launch`; protocol fee 25 SOL → vault; `acc += 75 × 10⁹ × 1e18 / 1e6 = 75 × 10²¹`; `reward_reserve = 75 × 10⁹ lamports` |
| 4 | Alice `claim_rewards` | pending = `1e6 × 75e21 / 1e18 = 75 × 10⁹ lamports = 75 SOL` |

Alice receives 75 SOL from a 100 SOL backlog with a 1-token stake.

---

## Trust model implications

This is benign **if**:

- The team announces staking open in good faith before any meaningful fee
  accrual.
- The first staker is not pre-coordinated with the team.

This is **predatory** if:

- The launch's bonding curve is routed through the program's `fee_owner` PDA
  during a "stealth" period where staking is technically open but unannounced.
- An insider stakes a minimal amount before public announcement, collecting the
  pre-announce backlog.

The program cannot enforce one model over the other. The README does not
currently document this behaviour, so users have no way to evaluate whether a
given launch is operating fairly.

---

## Recommended actions

### Option A — Document the behaviour

Add a section to `programs/staking/README.md` describing the first-staker
backlog behaviour. Users can then evaluate whether a launch's stake-opening
timing was fair.

### Option B — Initial reward distribution control

On `initialize_launch`, record `creator_vault.lamports()` and the equivalent
PumpSwap-ATA balance. On the first `claim_pump_*creator_fees` (or first
`stake`), discount that amount from the reward sweep and route it elsewhere
(treasury, burn, or split across the first N stakers). Requires more state and
careful UX.

### Option C — Enforce a "first staker grace period"

Require a minimum number of stakers (or a minimum weight floor) before any
`claim_pump_*creator_fees` can apply rewards. Defers the distribution until
diluted across a few participants. UX-heavy.

### Option D — Accept the design

If the protocol's incentive design specifically rewards first movers, leave the
behaviour as is. Document it loudly.

**Recommended:** **Option A** at minimum. Decide between B/C/D based on
business intent.

---

## References

### Source files

| File | Relevance |
|------|-----------|
| `programs/staking/src/instructions/staking.rs` | `stake` clean-vault skip |
| `programs/staking/src/instructions/fees.rs` | All `claim_pump_*creator_fees` handlers |
| `programs/staking/src/rewards.rs` | `apply_rewards`, `apply_rewards_with_protocol_fee`, `sweep_fee_owner_to_launch` |

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial write-up. |
