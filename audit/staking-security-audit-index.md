# Staked Program — Security Audit Index

**Document version:** 1.0
**Date:** 2026-05-24
**Program:** `staked`
**Program ID:** `aZPKJ99yjfE6MjZ5isXNjm3ndanBdx5gX35nwmLBANK`
**Repository:** [iceypump/staked](https://github.com/iceypump/staked)
**Official audit status (per README):** Unaudited — not recommended for production without independent review.

This document indexes individual finding write-ups produced from a code review of the
`staked` Anchor program at commit corresponding to the deployed build (program ID above).
Severity ratings follow the standard High / Medium / Low / Info hierarchy and are scoped
to economic impact on a live, mainnet-deployed instance with real reward flow.

---

## Findings at a glance

| ID | Severity | Title | Affected area | Funds at risk? |
|----|----------|-------|---------------|----------------|
| [HIGH-01](staking-security-audit.md) | **High** | Stale `QuoteStakeState` survives `unstake` and enables drain of quote rewards after re-stake | `unstake`, `claim_rewards` (quote path) | **Yes** — quote reward vaults |
| [MED-01](staking-security-audit-medium-locked-quote-rewards.md) | **Medium** | Quote rewards permanently lock when stakers never call `initialize_quote_stake` | `apply_quote_rewards`, `initialize_quote_stake` | Partial — reserve dust permanently stranded |
| [MED-02](staking-security-audit-medium-increase-stake-quote-desync.md) | **Medium** | `increase_stake` desynchronizes `QuoteStakeState`, under-paying the user | `increase_stake`, `claim_rewards` (quote path) | User loss, reserve grows |
| [LOW-01](staking-security-audit-low-protocol-fee-accounting-drift.md) | **Low** | `claim_quote_protocol_fees` silently zeroes `protocol_fee_reserve` and accepts unsolicited ATA deposits | `claim_quote_protocol_fees` | Accounting only (privileged caller) |
| [LOW-02](staking-security-audit-low-wsol-rent-residue.md) | **Low** | Rent-exempt residue from WSOL ATA close accumulates as unaccounted lamports in `launch` | `claim_pumpswap_creator_fees`, `claim_pumpswap_shared_creator_fees` | Stranded dust per claim |
| [INFO-01](staking-security-audit-info-first-staker-backlog.md) | **Info** | First staker can claim 100% of any creator-fee backlog accumulated before staking began | `stake`, `claim_pump_*creator_fees` | Design observation |
| [INFO-02](staking-security-audit-info-permissionless-init.md) | **Info** | Permissionless `initialize_launch` and `initialize_quote_rewards` | `initialize_launch`, `initialize_quote_rewards` | Design observation |

---

## Methodology

- Static review of every instruction handler in `programs/staking/src/instructions/`
  cross-referenced against account-context structs in
  `programs/staking/src/account_contexts.rs`.
- PDA derivation review against `programs/staking/src/pda.rs` and the constants in
  `programs/staking/src/constants.rs` (Pump.fun program IDs, instruction
  discriminators, account layout offsets).
- Reward accounting reviewed against the well-known
  `acc_reward_per_weight` / `reward_debt` (MasterChef / Synthetix) pattern, with
  attention to `u128` overflow surfaces and integer truncation.
- External-CPI validation reviewed for `invoke_signed` callers (Pump.fun creator-fee
  collection, PumpSwap creator-fee transfer, SPL Token transfer, system program
  transfers).
- Unit-test file `programs/staking/src/tests.rs` and the inline test module in
  `programs/staking/src/instructions/fees.rs` exercised to confirm intended layouts.

No fuzzing, symbolic execution, or formal verification was performed. No off-chain
helper script (`scripts/*.mjs`) is in scope; only the on-chain program is reviewed.

---

## Severity definitions

| Severity | Definition |
|----------|-----------|
| High | Direct loss or unauthorized extraction of user funds on a deployed program. Exploit requires no privilege. |
| Medium | Loss or lock-up of funds in non-trivial amounts, OR an exploit requiring conditions that are likely in production. |
| Low | Accounting drift, dust loss, or weaknesses that do not directly enable theft. |
| Info | Design observations and intentional trade-offs worth confirming. |

---

## Document history

| Version | Date | Notes |
|---------|------|-------|
| 1.0 | 2026-05-24 | Initial index; HIGH-01 already published as `staking-security-audit.md` v1.0. |
