# Staked Program — LOW-01: `claim_quote_protocol_fees` silently zeroes `protocol_fee_reserve` and accepts unsolicited ATA deposits

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
7. [Scenarios](#scenarios)
8. [What is NOT affected](#what-is-not-affected)
9. [Recommended fixes](#recommended-fixes)
10. [References](#references)

---

## Executive summary

`claim_quote_protocol_fees` sweeps the entire token balance of the
`quote_protocol_fee_vault_ata` to the program upgrade authority, and decrements
`reward_pool.protocol_fee_reserve` by that amount using `checked_sub(...).unwrap_or(0)`.

Two issues result:

1. **`unwrap_or(0)` hides accounting drift.** If `amount > protocol_fee_reserve`,
   the subtraction silently clamps to zero instead of erroring. This can happen
   any time the ATA balance exceeds what the program has accounted for —
   notably, when anyone sends tokens **directly** to the
   `quote_protocol_fee_vault_ata` (it is a regular SPL token account whose
   authority is a program-derived PDA, but it can receive transfers from any
   source).
2. **The handler trusts the ATA balance over the protocol's own ledger.** The
   amount swept is read from `token_account_amount(quote_protocol_fee_vault_ata)`,
   not from `reward_pool.protocol_fee_reserve`. Any donated or
   accidentally-routed tokens in that ATA are routed to the upgrade authority on
   the next call.

Combined effect: a privileged caller can extract more tokens than the program
ledger reflects, with no on-chain trace of the discrepancy beyond the silent
`unwrap_or(0)` saturation.

---

## Severity and impact

| Attribute | Assessment |
|-----------|------------|
| **Severity** | **Low** |
| **Likelihood** | Low in normal operation (ATAs are not commonly used as a donation target). |
| **Exploit complexity** | Trivial — anyone can send tokens to the ATA address; the upgrade authority claims them. |
| **Funds at risk** | Only tokens **voluntarily** sent to the ATA. Not other users' rewards. |
| **Privilege required** | `claim_quote_protocol_fees` requires program upgrade authority. The caller is trusted. |

The economic impact is limited because:

- The `quote_protocol_fee_vault_ata` is owned by a PDA controlled by the program;
  there is no legitimate reason for users to send tokens there.
- The caller must be the upgrade authority — a privileged actor.

This is filed as Low because it is accounting drift behind a trusted role, not a
path to draining user funds.

---

## Affected scope

### Affected instruction

| Instruction | Role |
|-------------|------|
| `claim_quote_protocol_fees` | Sweeps `quote_protocol_fee_vault_ata` balance to upgrade authority; decrements `protocol_fee_reserve` with `unwrap_or(0)` |

### Not affected

- `claim_protocol_fees` (SOL path) does not have an equivalent ledger field; it
  reads `protocol_fee_vault.lamports() − rent_floor` and transfers via system
  program. No `unwrap_or(0)` involved.

---

## Background

`apply_quote_rewards_with_protocol_fee` carves a 25% protocol fee
(`PROTOCOL_FEE_BPS = 2_500`) out of every incoming quote-fee batch:

- The fee tokens are routed via SPL transfer from `fee_owner_token_ata` to
  `quote_protocol_fee_vault_ata` (whose authority is the `quote-protocol-fees`
  PDA).
- `reward_pool.protocol_fee_reserve += protocol_fee` records the expected ledger
  balance.

The expectation is that
`protocol_fee_reserve == token_account_amount(quote_protocol_fee_vault_ata)` at
all times. `claim_quote_protocol_fees` is the privileged sweep that drains both.

---

## Root cause

### 1. `unwrap_or(0)` swallows ledger drift

```68:74:programs/staking/src/instructions/protocol.rs
    let reward_pool = &mut ctx.accounts.quote_reward_pool;
    reward_pool.protocol_fee_reserve = reward_pool
        .protocol_fee_reserve
        .checked_sub(amount)
        .unwrap_or(0);
```

If `amount > protocol_fee_reserve`, the subtraction underflows; `unwrap_or(0)`
clamps the new value to zero. The on-chain record now lies — it says the reserve
is fully drained when the truth is that the protocol just paid out an unaccounted
surplus.

### 2. Sweep amount comes from the ATA balance, not the ledger

```46:48:programs/staking/src/instructions/protocol.rs
    let amount =
        token_account_amount(&ctx.accounts.quote_protocol_fee_vault_ata.to_account_info())?;
    require!(amount > 0, PobError::NoRewards);
```

If anyone transfers extra tokens directly into the ATA (which is permitted by the
SPL Token program — ATAs are normal accounts that accept incoming transfers from
any signer), the `amount` swept exceeds the legitimate `protocol_fee_reserve`.
The surplus goes to the upgrade authority. The ledger is silently truncated.

---

## Vulnerable code paths

### Full handler

```12:75:programs/staking/src/instructions/protocol.rs
pub(crate) fn claim_quote_protocol_fees(ctx: Context<ClaimQuoteProtocolFees>) -> Result<()> {
    require_program_upgrade_authority(
        &ctx.accounts.program.to_account_info(),
        &ctx.accounts.program_data.to_account_info(),
        &ctx.accounts.authority.key(),
    )?;
    require_quote_reward_pool(/* ... */)?;
    require_keys_eq!(
        ctx.accounts.quote_protocol_fee_authority.key(),
        quote_protocol_fee_authority_pda(/* ... */).0,
        PobError::InvalidProtocolFeeVault
    );
    require_associated_token_account(/* quote_protocol_fee_vault_ata */)?;
    require_associated_token_account(/* authority_quote_ata */)?;

    let amount =
        token_account_amount(&ctx.accounts.quote_protocol_fee_vault_ata.to_account_info())?;
    require!(amount > 0, PobError::NoRewards);

    let signer_seeds: &[&[&[u8]]] = &[&[
        b"quote-protocol-fees",
        launch_key.as_ref(),
        quote_mint.as_ref(),
        &[ctx.bumps.quote_protocol_fee_authority],
    ]];
    transfer_spl_tokens(
        &ctx.accounts.quote_token_program.to_account_info(),
        &ctx.accounts.quote_protocol_fee_vault_ata.to_account_info(),
        &ctx.accounts.authority_quote_ata.to_account_info(),
        &ctx.accounts.quote_protocol_fee_authority.to_account_info(),
        amount,
        &ctx.accounts.quote_reward_pool.quote_token_program,
        signer_seeds,
    )?;

    let reward_pool = &mut ctx.accounts.quote_reward_pool;
    reward_pool.protocol_fee_reserve = reward_pool
        .protocol_fee_reserve
        .checked_sub(amount)
        .unwrap_or(0);

    Ok(())
}
```

---

## Scenarios

### Scenario A — Donation routed to upgrade authority

1. A user (or an automated process) mistakenly transfers 100 USDC to the
   `quote_protocol_fee_vault_ata` thinking it is the staking reward vault.
2. The upgrade authority calls `claim_quote_protocol_fees`.
3. The handler sweeps the **entire** 100 USDC plus any legitimate fee reserve.
4. `protocol_fee_reserve` is decremented by the full amount; if 100 USDC exceeds
   the legitimate reserve, `unwrap_or(0)` clamps it to zero.
5. The donated 100 USDC ends up in the upgrade authority's wallet without any
   on-chain accounting evidence of the discrepancy.

### Scenario B — Pre-funding race

1. Upgrade authority sends 1 USDC to the ATA before any legitimate fees flow.
2. First `claim_pump_quote_creator_fees` adds 0.5 USDC to the ATA and bumps
   `protocol_fee_reserve` to 0.5.
3. `claim_quote_protocol_fees` sweeps 1.5 USDC; `protocol_fee_reserve` is
   `checked_sub(1.5).unwrap_or(0) = 0`.
4. The ledger says zero, but the actual swept amount was 3× the recorded reserve.

### Scenario C — Honest case (no drift)

If no extraneous deposits ever occur, `amount == protocol_fee_reserve` and the
`checked_sub` never underflows. The `unwrap_or(0)` is a no-op. The bug is dormant.

---

## What is NOT affected

| Asset / flow | Reason |
|--------------|--------|
| User staked principal | Separate account; not touched |
| User SOL rewards | Separate accounting; uses lamport-level transfers |
| User quote rewards (`reward_reserve`) | Separate ledger field; not subject to this `unwrap_or(0)` |
| `claim_protocol_fees` (SOL) | Reads `lamports() − rent_floor`; no equivalent ledger drift |

---

## Recommended fixes

### Option A — Require ledger consistency (preferred)

Replace the sweep-by-ATA-balance pattern with a sweep-by-ledger pattern:

```rust
let amount = reward_pool.protocol_fee_reserve;
require!(amount > 0, PobError::NoRewards);
// transfer exactly `amount`
reward_pool.protocol_fee_reserve = 0;
```

This pays out exactly what the program has accounted for. Any donated surplus
remains in the ATA and is not paid out. A separate `sweep_donated_dust`
instruction can be added if recovery of donations is desired.

### Option B — Hard error on underflow

Replace `unwrap_or(0)` with `?`:

```rust
reward_pool.protocol_fee_reserve = reward_pool
    .protocol_fee_reserve
    .checked_sub(amount)
    .ok_or(PobError::MathOverflow)?;
```

The transaction reverts if the ATA balance exceeds the ledger. Privileged callers
must investigate before sweeping. Donations cannot be silently absorbed.

### Option C — Both A and B

Use the ledger value as the transfer amount and replace `unwrap_or(0)` with `?`
as a defensive measure.

**Recommended:** **Option A** — sweep by ledger. It is the simplest invariant
and makes the program's intent explicit.

---

## References

### Source files

| File | Relevance |
|------|-----------|
| `programs/staking/src/instructions/protocol.rs` | `claim_quote_protocol_fees` |
| `programs/staking/src/rewards.rs` | `apply_quote_rewards_with_protocol_fee` (writes the ledger) |
| `programs/staking/src/token.rs` | `token_account_amount` |
| `programs/staking/src/constants.rs` | `PROTOCOL_FEE_BPS = 2_500` |

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial write-up. |
