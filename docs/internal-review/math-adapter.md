# Internal security review: Jupiter adapter and curve arithmetic

Date: 2026-10-29. Reviewer: internal AI-assisted review, not an independent third-party audit or certification.

## Scope and result

Reviewed `evolut-jupiter-amm/src/lib.rs`, `src/quote.rs`, the vendored native swap entrypoints, exact-input/partial/output arithmetic, curve arithmetic helpers, fee mode/configuration/split functions, pool reserve accounting, and relevant configuration/initialization constraints. The vendor and runtime implementation were not changed. No transactions or account mutations were sent to any network.

No reproducible critical, high, or medium fund-loss vulnerability was identified in this bounded math/adapter review. This statement is limited to the supported legacy-SPL/WSOL, flat 1%/2%, ExactIn path and the tests described below; it is not a guarantee that the entire contract is safe.

Reviewed baseline: local standalone commit `3e8e5082af9dff53d2e1b7792d8348c9d0d04e62`, with new uncommitted audit-only tests. The deployed-program fixture is the captured 1,207,880-byte SBF binary with SHA-256 `885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6`.

## Confirmed integration limitation — MA-I01

**Severity: Informational / release dependency. Confirmed, not an exploit.**

`src/lib.rs:436` returns `Swap::Placeholder { data: None }`. The native instruction and account layout are implemented and tested, but this does not itself add Evolut to Jupiter's production router. Jupiter must supply its real swap variant/dispatch integration. The executed SBF tests are top-level native swaps; the existing transfer-authority test verifies CPI-compatible account metas, not an actual Jupiter-router CPI invocation.

Required before claiming production Jupiter routing: agree the router variant/ABI with Jupiter and exercise a real router CPI using its transfer authority, including account privilege forwarding, slippage rejection and failure rollback. Do not describe this repository as already routed by Jupiter.

## Arithmetic and accounting verified

- Fees are charged in WSOL on input for buys and on gross WSOL output for sells. Native code uses ceiling division by 1,000,000,000 (`state/config.rs:160`); the adapter reports the fee in WSOL (`src/quote.rs:80`). Independent integer calculations in tests agree for 1% and 2%.
- Creator share is `floor(fee / 2)`; the partner receives the remainder. For an odd fee, the difference is one lamport. No fee disappears in rounding. The reviewed build's protocol/referral percentages are both zero (`constants.rs:96`).
- WSOL fees stay inside the quote vault but outside the tradable quote reserve. `apply_swap_result` updates reserve and fee balances separately (`state/virtual_pool.rs:859`). The audit checks actual wallet debit/credit, vault deltas, reserve deltas, accumulated fee metrics and both recipient counters after executed buys and sells.
- ExactIn refuses an input that cannot be fully consumed before the migration price. The adapter additionally rejects a zero-output result. A microscopic two-lamport buy can acquire tokens whose immediate resale rounds to zero WSOL; the adapter deliberately does not route that sale. This is unit rounding, not proof of missing funds.
- Completion checks preserve the base inventory required for migration, vesting and accrued base fees (`src/quote.rs:55`; native `ix_swap.rs:294`). Native graduation preserves historical quote reserves, so the native completed-curve check continues to prevent curve trading after graduation.
- Fee/reserve arithmetic uses checked additions/subtractions and widened multiplication. Unsellable inputs beyond available quote reserve fail instead of consuming the fee liabilities. Unsupported ExactOut/partial fills, Token-2022, referral, dynamic-fee and scheduled-fee modes are rejected by the adapter.

## Account validation and cache behavior reviewed

- The constructor/update validate program owner, executable bit, exact ABI lengths, discriminators, pool PDA and both vault PDAs (`src/lib.rs:93`, `:112`).
- Both mints and both vaults are checked as legacy SPL accounts; mint decimals, initialization, freeze authority, vault mint/authority, frozen/delegated/close-authority/native-token state are checked (`src/lib.rs:213`, `:230`).
- Vault amounts must cover reserves plus all accrued fee balances, with overflow causing rejection (`src/lib.rs:387`).
- A failed refresh clears `ready` and `config` before reading new data, preventing continued quotes from a previously ready snapshot (`src/lib.rs:361`). The dependency list is refreshed after a newly decoded pool.
- The clock is consulted for every quote/build eligibility check. Active, completed and migrated states are distinguished. Slippage is encoded as the caller's minimum output and checked by native execution.
- Supporting a router transfer authority different from the user/payer is intentional. The adapter does not require associated token accounts; enforcing that would incorrectly reject legitimate routed token accounts. SPL Token enforces actual transfer authority at execution.

## New adversarial evidence

New file: `evolut-jupiter-amm/tests/security_invariants.rs`. The existing execution harness was copied into this new file only; vendor/runtime and existing tests were untouched.

The supervising agent confirmed the final complete suite passed: 27 tests, including all four new audit tests after the dead-helper removal and explicit sell-case-count assertion. The root audit evidence log records the command and output.

1. **Round trips and solvency:** 28 input sizes for each of two fee tiers (56 buys). These include fee rounding boundaries, two-lamport dust, deterministic varied amounts and 84 SOL close to graduation. There are 27 positive-output return sells per tier (54 sells); the dust resale is intentionally unroutable. Every completed round trip must return no more WSOL than it spent and restore the starting base-token balance. Actual fee ledgers and vault coverage are asserted after each execution.
2. **Integer-range quote probes:** 16 explicit boundaries plus 128 deterministic u64 samples, both directions: **288 quote probes** after one executed buy. Probes cover zero, tiny amounts, near-threshold amounts and `u64::MAX`. Quotes must either reject or return without a panic; every accepted quote must preserve ExactIn and the independently computed fee.
3. **Fee conservation:** **28 independent vectors** across 1% and 2%, including `u64::MAX`. Assert ceil fee, net-plus-fee conservation, exact 50/50 integer split, and at most one lamport of remainder to the partner.
4. **Segment transitions:** a synthetic local subdivision of the captured curve into two equal-liquidity segments. A 70 SOL buy crosses the boundary and returning all bought tokens crosses it in the opposite direction. Both execute against the captured SBF with exact quoted output and conserved fee ledgers. This is synthetic local coverage, not a claim that this account configuration exists on mainnet.

The tests execute **113 native SBF swaps** in total: 110 round-trip buys/sells, one setup buy for the quote probes, and two segment-crossing swaps. Token-account creation transactions are additional setup and not counted as swaps. Original tests separately cover slippage rollback and above-threshold rejection.

## Residual risks and limitations

- No actual production Jupiter CPI route, multihop transaction, malicious router or real delegated-token swap was executed. Native parity alone cannot establish those integration properties.
- The native wrapper does not separately check activation time or migration flags. Current initialization sets activation to the current clock, and graduation preserves a completed quote reserve; no reachable bypass was found with these rules. If future code permits scheduled activation or resets reserves, add explicit native gating before deploying that change. Do not report this as a currently exploitable vulnerability without a reachable state.
- Low-level vendored math contains `assert!` and unchecked range subtractions that assume positive liquidity and ordered prices. The adapter validates nonzero/ordered active curve segments, and reviewed swap branches maintain those preconditions. Boundary probes did not identify a reachable panic. Directly invoking a helper with impossible arguments is not a deployed exploit; exhaustive malformed-state fuzzing has not been completed.
- The adapter's validation is a swap-support filter, not a full revalidation of every graduation policy field. Some malformed synthetic program-owned states could still pass initial update and fail later. On-chain program ownership and immutable validated config creation are part of the trust boundary.
- The 1% SBF case changes only the local captured config fee numerator. It is explicitly synthetic, rather than a captured 1% pool. The multisegment case is also synthetic.
- No formal proof, exhaustive state-space analysis, long-running fuzz campaign, independent economic audit, or independent SBF-to-source reproducible-build attestation was completed in this sub-review. A byte-matching fixture is evidence of the tested binary, not by itself a proof of its source provenance.

## Confirmed dependency finding — MA-L02

**Severity: Low in the reviewed supported path / dependency remediation required. Status: affected versions and dependency drift confirmed; a reachable fund-loss or crash exploit was not demonstrated.**

The reviewed contract lockfile (`vendor/evolut-curve/Cargo.lock:1759`) pins `ruint` 1.14.0, whereas the standalone adapter root lock (`Cargo.lock:2831`) resolves 1.17.2. Thus identical vendored Rust math source does not imply an identical dependency graph. The 113 native-SBF swaps and existing parity vectors compare the actual binary with the adapter and passed, but are not exhaustive.

Public advisories were checked against the official RustSec pages and locally cached crate source:

- [RUSTSEC-2025-0137](https://rustsec.org/advisories/RUSTSEC-2025-0137.html): the old public `reciprocal_mg10` helper can index outside its table if directly called with a non-normalized divisor. Fixed in 1.17.1; the contract lock is affected, adapter 1.17.2 is not.
- [RUSTSEC-2026-0220](https://rustsec.org/advisories/RUSTSEC-2026-0220.html): incorrect shift overflow flags and extremely large wrapping shift amounts. Fixed in 1.20.0; both locked versions are affected.

### Reachability assessment: reciprocal advisory

No Evolut source directly invokes `ruint::algorithms`, `reciprocal_mg10`, or normalized-division entrypoints. The reviewed call chain uses high-level `checked_div`, `div_rem` or `div_ceil` on U256/U512 values. In cached ruint 1.14.0:

1. `src/div.rs:53` dispatches `div_rem` through `algorithms::div`.
2. `src/algorithms/div/mod.rs:48` trims zero high limbs, rejects a zero divisor, and dispatches by nonzero divisor length.
3. Single-limb division normalizes with `leading_zeros` and a primitive-u64 left shift before reciprocal (`small.rs:55-62`). Two-limb division similarly normalizes the top limb (`small.rs:125-134`). Multi-limb division normalizes its leading double-word before reciprocal (`knuth.rs:99-114`).
4. These paths establish the required top bit before the table lookup. The unsafe public-helper advisory is real, but its invalid-input trigger was not found through Evolut's high-level division calls. All contract denominators are nonzero by validation or return a checked error before division.

This is a source-based reachability assessment, not a proof covering arbitrary future dependencies or direct calls.

### Reachability assessment: shifts advisory

The advisory states that ordinary shifted values remain correct; the defect concerns overflow reporting, plus wrapping shifts of at least 2^32. Current reviewed usage does not depend on a faulty overflow flag in an overflowing left shift:

| Site | Input and shift bound | Assessment |
| --- | --- | --- |
| `curve.rs:49`, `:202`, `:300`; `params/liquidity_distribution.rs:72` | U256 made from u64, shifted 128 bits | At most 192 significant bits; fits U256, no discarded high limbs |
| `curve.rs:149` | U256(1) shifted 128 | Fits U256 |
| `state/fee.rs:51` through `math/u128x128_math.rs:30` | U256 made from u128, shifted 64 | At most 192 bits; dynamic fees additionally unsupported in this adapter/current new configs |
| `curve.rs:154`, `math/u128x128_math.rs:19` | Right shift for intentional floor rounding; returned flag discarded | Shifted integer is correct under the cited advisory |
| `math/utils_math.rs:81` | U256(1), shift derived from leading zeros and bounded to 0..254 | Fits U256; no untrusted huge shift |
| `math/utils_math.rs:89-93` | Right shifts 1 or 2 | No whole 64-bit limb discarded; this helper is used by unsupported rate-limiter mode |

Only limb-aligned U256/U512 ruint types are used in Evolut source. No U160-like type or attacker-controlled shift of at least 2^32 was found. `fee_math.rs` uses primitive u128 shifts, not ruint shifts. The generic `shl_div_256` accepts a u8 offset but has no production caller in the reviewed tree; callers of `shl_div` use the constant 64. No live-path false-overflow acceptance was established from these advisories.

### Remediation and release sequencing

Track the dependency finding rather than suppressing it as a clean scan. Prepare a coordinated upgrade to a tested patched ruint version (at least 1.20.0, subject to Rust/SBF toolchain compatibility) in both the contract build and adapter. Build a new SBF artifact, rerun quote/native parity, adversarial amount/segment/fee tests, and the full contract suite, then use the normal owner-signed upgrade process. Keep old deployed-artifact fixtures while separately testing the new candidate; do not silently replace the fixture or imply the running program changed after editing a lockfile. No dependencies, live program, or lockfiles were changed in this review.
