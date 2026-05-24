# Staked Program — LOW-02: Rent-exempt residue from WSOL ATA close accumulates as unaccounted lamports in `launch`

**Document version:** 1.0
**Date:** 2026-05-24
**Program:** `staked`
**Program ID:** `aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK`
**Severity:** **Low**
**Status:** Unfixed at the time of writing.

---

## Table of contents

1. [Executive summary](#executive-summary)
2. [Severity and impact](#severity-and-impact)
3. [Affected scope](#affected-scope)
4. [Background](#background)
5. [Root cause](#root-cause)
6. [Vulnerable code paths](#vulnerable-code-paths)
7. [Numerical example](#numerical-example)
8. [Compounding behaviour](#compounding-behaviour)
9. [What is NOT affected](#what-is-not-affected)
10. [Recommended fixes](#recommended-fixes)
11. [References](#references)

---

## Executive summary

`claim_pumpswap_creator_fees` and `claim_pumpswap_shared_creator_fees` collect
PumpSwap creator fees as wrapped-SOL into a temporary `fee_owner_wsol_ata`, then
close that ATA into the `launch` account to materialize the SOL. The amount
applied as a reward is read **before** the close from
`fee_owner_wsol_ata.lamports()`.

`AccountInfo.lamports()` returns the **total** lamports on the account — i.e., the
WSOL token balance plus the **rent-exempt minimum** required to keep the SPL
token account alive. Closing the account transfers that full lamport total into
`launch`. The handler then accounts for the full transferred amount as a reward
batch via `apply_rewards_with_protocol_fee`.

This is internally consistent — the rewards distributed match the lamports
received. **However**, the rent-exempt minimum was originally paid by the
**payer** of the ATA's creation (typically the user or a prior transaction), not
by the protocol. Treating that one-time rent contribution as reward inflates
`launch.reward_reserve` by the rent-exempt minimum on every claim, and grows
`launch.lamports()` proportionally.

The compound effect: every call adds ≈ 0.00204 SOL of rent-floor lamports into the
launch account's "rewards." Over many claims this is real money attributed to
stakers from a source other than creator fees.

A second observation: there is **no instruction** that distinguishes
"reward-reserve lamports" from "other lamports on the launch account." If the
launch ever receives lamports from a non-reward source (e.g., direct transfer),
those are also paid out via `pay_rewards`. The bug is direction-symmetric.

---

## Severity and impact

| Attribute | Assessment |
|-----------|------------|
| **Severity** | **Low** |
| **Likelihood** | Every PumpSwap creator-fee claim triggers it. |
| **Exploit complexity** | None — this is a passive accounting drift, not an attack. |
| **Funds at risk** | Per claim: ≈ 0.002 SOL (the SPL token account rent-exempt minimum) of "rewards" sourced from the original ATA payer rather than from creator fees. |

Direction: this is a **net inflow** to the staking reserve, paid for by whoever
funded the WSOL ATA's rent. In the present codebase that is the same `payer`
signer who is calling the claim instruction — so the inflow is voluntary and
self-imposed. It is still worth fixing because:

- The accounting is misleading (rent contributions appear as "rewards").
- Adversarial framing: anyone could over-fund the WSOL ATA before the claim and
  have the surplus distributed as rewards. This is benign for honest claims but
  becomes a routing mechanism for unwanted lamports.

---

## Affected scope

### Affected instructions

| Instruction | Role |
|-------------|------|
| `claim_pumpswap_creator_fees` | Closes `fee_owner_wsol_ata` into `launch`, applies full lamport delta as reward |
| `claim_pumpswap_shared_creator_fees` | Same pattern for shared (sharing-config) launches |

### Not affected

- `claim_pump_creator_fees` (Pump.fun direct SOL path) — uses
  `sweep_fee_owner_to_launch`, which subtracts a `rent_floor` before sweeping.
  This is the correct pattern.
- All quote-token paths — they use SPL token balances measured with
  `token_account_amount`, not lamports.

---

## Background

Both PumpSwap creator-fee paths collect fees as wrapped-SOL because PumpSwap's
`COLLECT_COIN_CREATOR_FEE` instruction deposits into an SPL Token account, not a
system account. To deliver those fees as native SOL into the launch (so they can
be tracked as `reward_reserve` and paid out via `move_lamports`), the program:

1. Calls the PumpSwap CPI; lamports backing the WSOL balance land in
   `fee_owner_wsol_ata`.
2. Reads `fee_owner_wsol_ata.lamports()` into `reward_lamports`.
3. Closes the ATA (CPI to SPL Token `CloseAccount` with the `fee_owner` PDA as
   authority); destination is `launch.to_account_info()`. SPL `CloseAccount`
   transfers **all** lamports of the closed account to the destination.
4. Calls `apply_rewards_with_protocol_fee(launch, ..., reward_lamports)` — this
   credits `launch.reward_reserve += reward_lamports − protocol_fee` and bumps
   `acc_reward_per_weight`.

The ATA's lamports on close are: `WSOL_token_balance + rent_exempt_minimum`.

---

## Root cause

The handler attributes the **entire lamport delta** received by `launch` (token
amount **plus** rent-floor) to the creator-fee reward. The rent-floor portion is
not creator-fee income; it is the rent contribution that originally created the
ATA.

---

## Vulnerable code paths

### 1. `claim_pumpswap_creator_fees`

```516:540:programs/staking/src/instructions/fees.rs
let collected_wsol = token_account_amount(&ctx.accounts.fee_owner_wsol_ata.to_account_info())?;
require!(collected_wsol > 0, PobError::NoRewards);

let reward_lamports = ctx.accounts.fee_owner_wsol_ata.to_account_info().lamports();
close_spl_token_account(
    &ctx.accounts.quote_token_program.to_account_info(),
    &ctx.accounts.fee_owner_wsol_ata.to_account_info(),
    &ctx.accounts.launch.to_account_info(),
    &ctx.accounts.fee_owner.to_account_info(),
    &[&[
        b"fee-owner",
        ctx.accounts.launch.mint.as_ref(),
        &[ctx.bumps.fee_owner],
    ]],
)?;
require!(reward_lamports > 0, PobError::NoRewards);

apply_rewards_with_protocol_fee(
    &mut ctx.accounts.launch,
    &ctx.accounts.protocol_fee_vault.to_account_info(),
    &ctx.accounts.payer.to_account_info(),
    &ctx.accounts.system_program.to_account_info(),
    ctx.bumps.protocol_fee_vault,
    reward_lamports,
)?;
```

`reward_lamports = WSOL_amount + rent_exempt_minimum`. The full amount is applied
as a reward.

### 2. `claim_pumpswap_shared_creator_fees`

Same pattern, with `transfer_pumpswap_creator_fees_to_pump_v2` and
`distribute_pump_creator_fees_v2` upstream of the close. Lines:

```759:783:programs/staking/src/instructions/fees.rs
let collected_wsol = token_account_amount(&ctx.accounts.fee_owner_wsol_ata.to_account_info())?;
require!(collected_wsol > 0, PobError::NoRewards);

let reward_lamports = ctx.accounts.fee_owner_wsol_ata.to_account_info().lamports();
close_spl_token_account(/* ... */)?;
require!(reward_lamports > 0, PobError::NoRewards);

apply_rewards_with_protocol_fee(
    &mut ctx.accounts.launch,
    /* ... */,
    reward_lamports,
)?;
```

### 3. Contrast: Pump.fun direct path subtracts rent floor

```299:326:programs/staking/src/rewards.rs
pub(crate) fn ensure_fee_owner_rent_exempt<'info>(/* ... */) -> Result<u64> {
    let rent_floor = Rent::get()?.minimum_balance(0);
    // ...
    Ok(rent_floor)
}
```

```268:297:programs/staking/src/rewards.rs
pub(crate) fn sweep_fee_owner_to_launch<'info>(
    // ...
    rent_floor: u64,
) -> Result<u64> {
    let amount = fee_owner
        .lamports()
        .checked_sub(rent_floor)
        .ok_or(PobError::MathOverflow)?;
    // ... system_program::transfer of `amount` ...
}
```

The non-PumpSwap paths correctly subtract the rent floor before applying the
reward. The PumpSwap paths do not.

---

## Numerical example

SPL Token account rent-exempt minimum at typical rent rates:
~**0.00203928 SOL** (2,039,280 lamports).

Suppose a PumpSwap creator-fee claim collects **1.000 SOL** of WSOL:

| Quantity | Value |
|----------|-------|
| `collected_wsol` (token amount) | 1,000,000,000 lamports |
| ATA lamports before close | 1,000,000,000 + 2,039,280 = 1,002,039,280 |
| `reward_lamports` | 1,002,039,280 |
| 25% protocol fee | 250,509,820 |
| Net to `reward_reserve` | 751,529,460 |
| Expected (token only) net | 750,000,000 |
| **Inflated by** | **1,529,460 lamports** ≈ **0.00153 SOL** |

Each subsequent claim inflates by another rent-floor minus the protocol fee
share of it.

---

## Compounding behaviour

Across N PumpSwap claims:

```text
total_inflated_lamports ≈ N × rent_exempt_minimum × (1 − PROTOCOL_FEE_BPS / 10_000)
                        = N × 2_039_280 × 0.75
                        ≈ N × 1_529_460 lamports
```

For N = 100 claims: ≈ 0.153 SOL inflated into rewards. For N = 1,000 claims:
≈ 1.53 SOL. Real but not catastrophic.

The matching protocol-fee share also accumulates in the `protocol_fee_vault`
PDA (as a system account); `claim_protocol_fees` will sweep that as
`lamports() − rent_floor`, so the protocol authority eventually receives the
25% share of the rent-floor inflows as additional fee revenue.

---

## What is NOT affected

| Asset / flow | Reason |
|--------------|--------|
| User principal | Stored in `staking_vault` (SPL token account), independent |
| Pump.fun SOL creator fees | `sweep_fee_owner_to_launch` correctly subtracts rent floor |
| All quote-token paths | Use SPL token balances, not lamport balances |
| Total network lamports | Conserved — the rent floor moves from payer → launch, not minted |

---

## Recommended fixes

### Option A — Subtract rent floor before applying reward (preferred)

Mirror the Pump.fun direct path:

```rust
let collected_wsol = token_account_amount(&fee_owner_wsol_ata.to_account_info())?;
require!(collected_wsol > 0, PobError::NoRewards);

let rent_floor = Rent::get()?.minimum_balance(TOKEN_ACCOUNT_LEN);
let ata_lamports = fee_owner_wsol_ata.to_account_info().lamports();
let reward_lamports = ata_lamports.checked_sub(rent_floor).ok_or(PobError::MathOverflow)?;

close_spl_token_account(/* ... destination = launch ... */)?;

apply_rewards_with_protocol_fee(launch, /* ... */, reward_lamports)?;
```

This applies exactly the token amount as the reward. The rent-floor lamports
still arrive at `launch` (via `CloseAccount` destination) but are **not** counted
as rewards — they become surplus lamports on `launch` that no one can claim.

To return the rent floor to the original payer, a follow-up `system_program::transfer`
from `launch` (signed by the `launch` PDA seeds) to the `payer` for exactly the
rent-floor amount can be added. This restores both correctness and rent
conservation.

### Option B — Close the ATA into the `payer`, not `launch`

Direct the `CloseAccount` destination to the original `payer`. The rent floor is
refunded to the caller. The ATA's WSOL balance, however, also goes to the payer
— so the program must first unwrap WSOL by another mechanism (or use a transfer
before close). This is more invasive and increases CPI count.

### Option C — Accept and document

Mark the residue as an intentional contribution to the reward reserve, and
document it. Acceptable only if the team decides it is preferable to the added
complexity of the fix. The accounting remains misleading.

**Recommended:** **Option A** with the optional rent-refund follow-up.

---

## References

### Source files

| File | Relevance |
|------|-----------|
| `programs/staking/src/instructions/fees.rs` | `claim_pumpswap_creator_fees`, `claim_pumpswap_shared_creator_fees` |
| `programs/staking/src/rewards.rs` | `sweep_fee_owner_to_launch`, `apply_rewards_with_protocol_fee` |
| `programs/staking/src/token.rs` | `close_spl_token_account`, `token_account_amount` |
| `programs/staking/src/constants.rs` | `TOKEN_ACCOUNT_LEN`, `PROTOCOL_FEE_BPS` |

### External references

- SPL Token `CloseAccount`: transfers all lamports from the closed account to
  the destination.
- Rent-exempt minimum for a `TOKEN_ACCOUNT_LEN = 165`-byte account ≈ 2,039,280
  lamports at current rent rates.

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial write-up. |
