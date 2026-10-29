# Internal security review: graduation and Raydium fee claims

Date: 2026-10-29. Reviewer: an AI-assisted internal review agent working for Evolut. **This is not an independent third-party audit, formal verification, or certification of safety.**

## Scope and identity

Reviewed source root: `<review-workspace>/evolut-jupiter-amm/vendor/evolut-curve/programs/dynamic-bonding-curve/src/`.

Reviewed graduation and fee-claim paths: `instructions/migration/{graduate,create_locker,flash_rent,withdraw_leftover}.rs`, `raydium.rs`, `instructions/creator/ix_claim_raydium_creator_fees.rs`, `state/raydium_creator_fees.rs`, and relevant `state/config.rs`, `instructions/partner/ix_create_config.rs`, `instructions/swap/ix_swap.rs`, and `utils/cpi_checker.rs` behavior. Broader swap arithmetic, launch authorization, metadata controls, backend, and Jupiter adapter are separate review scopes.

Program: `EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ`.

Local SBF tested: `output/matched-fee-contract-20261028/target/deploy-mainnet/dynamic_bonding_curve.so`, SHA-256 `885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6`. The hash was checked with `Get-FileHash` and asserted by the new VM tests. This sub-review did not re-fetch the live executable or independently reproduce the compiler build; the parent review supplies source/binary/live linkage evidence.

## Result

No confirmed permissionless theft, LP-principal withdrawal, cross-pool fee theft, replay payout, or graduation burn/reimbursement exploit was identified in this scoped review. This is a statement about the work performed, not proof that no vulnerability exists.

23 new adversarial tests passed against the actual pinned SBF using offline LiteSVM. Existing regression tests were inspected for their assertions; their fresh execution status is reported by the parent review separately. No live account, transaction, executable, or reviewed vendor source was changed.

## Findings and residual risks

### GC-01 — External policy changes can halt graduation after curve completion

**Severity: Medium availability risk; privileged external dependency, not a demonstrated permissionless exploit. Status: documented; no code change proposed without policy review.**

Evidence:

- `instructions/migration/graduate.rs:151` validates the current Raydium configuration; lines 153–156 require trade, protocol, fund, and creation fees to equal the values copied into the immutable Evolut configuration.
- `raydium.rs:141` onward checks the configuration's canonical PDA, enabled state and prescribed fee policy. `graduate.rs:167` verifies the current permission account for the 1% creator-fee route.
- `instructions/swap/ix_swap.rs:291` and lines 311–318 advance a completed curve into migration state before the separate graduation transaction.
- Existing VM test `mainnet-fee-routes.test.ts:362` mutates the external permission or creator fee rate, requires graduation failure, and proves no destination pool or partial virtual-pool state survives.

Preconditions: Raydium changes/disables the selected configuration, revokes the payer permission, changes a relevant fee, or upgrades an external ABI; alternatively, setup rent grows beyond the immutable cap. A random caller cannot rewrite these official program-owned accounts. Configuration drift is not itself evidence that funds were stolen.

Impact: a curve can finish trading while successful graduation remains unavailable until the external condition is repaired or a deliberately reviewed program upgrade adds recovery. The checked path has no automatic alternate Raydium configuration or return-to-curve mechanism. Failing closed avoids silently changing promised economics, but requires an operational response.

Recommended action: monitor official configuration/permission and setup estimates before allowing new launches and while curves are active; alert with the affected pool/config; retain an explicit incident procedure. Do not weaken account/fee checks simply to force graduation through. Monitor external program upgrades because pinned VM fixtures do not track future changes automatically.

### GC-02 — 50/50 is a split of net collected creator fees, not a guaranteed share of gross trade fees

**Severity: Informational economic/dependency assumption. Status: expected behavior, disclose accurately.**

Evidence:

- `instructions/creator/ix_claim_raydium_creator_fees.rs:91` calls the official permissionless creator-fee collection instruction; line 108 computes claims from the resulting per-pool account balances.
- `state/raydium_creator_fees.rs:19` reconstructs lifetime receipts as both sides' paid amounts plus the current balance. Lines 22–28 allocate floor(total/2) to the creator side and the odd atomic unit to the partner side.
- `raydium.rs:169` bounds the creator fee rate but does not freeze the external `creator_fee_share_rate` or an official per-creator share override.
- Existing tests at `mainnet-fee-routes.test.ts:339` exercise external creator-share overrides of 0, 333333, 1000000 and an invalid 1000001 parts per million. Valid overrides are split only after Raydium's deduction; an invalid rate rolls back.

Preconditions: the official Raydium configuration/override changes its share, or an operator describes a gross rate as a guaranteed net payout. An arbitrary claimant cannot bypass NFT ownership to collect another side's portion.

Impact: net creator/platform receipts can change without changing the nominal 1% trading label. With a hypothetical valid 100% external creator-share deduction, the creator bucket can pay zero even though the internal 50/50 ledger is correct. The tested fixture's 5% external share is synthetic; it is not a fresh quote of current mainnet policy.

Recommended action: display and document the current base fee, creator fee, external deduction and resulting net split separately. Monitor official overrides. Treat the fixed counterpart addresses as trust dependencies, not a promise that their policy can never change.

### GC-03 — Permanent LP custody depends on the official Raydium locker and its upgrade governance

**Severity: Informational trust boundary. Status: expected external dependency.**

Evidence:

- `graduate.rs:331` onward locks both LP portions through the fixed official locker, then checks both lock records, source LP balance zero, and aggregate locker LP balance.
- `graduate.rs:552` verifies the initial locked amount and zero claimed/unclaimed fees, pool, NFT and LP mint bindings, plus the NFT recipient.
- External fixture manifest `operations-rollout/server/vm-tests/external-fixtures.mainnet.json` records upgrade authorities for both CPMM and locker at its 2026-10-18 capture. This sub-review did not re-audit all external source or re-read their live upgrade authorities.

The NFT grants fee rights, not direct SPL authority over the locker vault. An ordinary holder's direct transfer of the locked LP is rejected in the existing SBF test at `mainnet-fee-routes.test.ts:278`. However, this is not a proof that every possible external locker instruction, future upgrade, or governance decision preserves principal. Do not advertise Evolut's own checks as elimination of external-program trust.

## Controls examined and supporting evidence

| Area | Control and evidence | Result / limitation |
| --- | --- | --- |
| CPI program substitution | Fixed executable CPMM/locker/metadata IDs; official owner/discriminator/exact-size decoders in `raydium.rs`; deterministic destination, vault, LP, lock, NFT and ATA checks in `graduate.rs:176–215`. | Code review plus new negative cases reject substituted program/authority/destination accounts atomically. |
| Creator and treasury substitution | Creator bound to `VirtualPool.creator`, partner to fixed treasury and `PoolConfig.fee_claimer`, at `graduate.rs:132–140` and account constraints. | Payer cannot redirect either initial fee NFT to themselves. Public graduation is safe only insofar as these fixed bindings and external programs remain correct. |
| Rent reimbursement | Fresh system-account checks and per-account rent shortfall in `raydium.rs:228`; forecast cap at `graduate.rs:218–239`; caller funds authority; measured consumption must equal forecast at `graduate.rs:359–366`; reimbursement is only to caller's existing native-WSOL ATA at lines 374–385. | A 1-lamport prefund reduces the measured reimbursement by exactly 1. Caller cannot request an arbitrary destination or amount. Cost changes fail closed. |
| Replay / partial failure | Initial migration state and empty receipt guard at `graduate.rs:119–123`; state finalized only after all CPIs and assertions at lines 428–454. | Existing replay and late-failure tests cover repeated graduation and rollback after CPMM/locker CPIs. New substituted-account cases preserve source pool, vault and mint bytes. |
| Treasury graduation revenue | Version 2 maps 0.5 SOL threshold to zero and 85 SOL threshold to 0.8 SOL at `state/config.rs:460–489`; quote deposit equals threshold less actual setup and the fixed fee; fee becomes `partner_quote_fee` at `graduate.rs:433`. | Fee is retained as treasury-claimable WSOL, not paid to worker or split as trading income. Version 1 deliberately remains zero fee. Existing tests assert accounting for both thresholds, both fee tiers and mint orderings. |
| LP split / lock | Creator gets floor(LP/2), partner the remainder; both lock CPIs complete in one transaction; source LP zero, locker LP total equal and pool bindings checked at `graduate.rs:324–373`. | Correct 50/50 allocation with at most one atomic LP unit rounding to partner. External locker fee accounting remains an external dependency. |
| Burn and supply | Ledger-based unused inventory, not raw donation-inclusive vault balance, at `state/config.rs:572–584`; protected fee balance and exact post-burn supply/vault deltas at `graduate.rs:396–426`. | New actual-SBF test proves an unsolicited base donation survives while unused issued inventory burns. New mainnet profiles have 1B initial tokens, six decimals, zero vesting and zero post-migration floor (`ix_create_config.rs:160–181`). The final supply can be smaller after graduation. |
| Leftover withdrawal | Receipt allocation, once-only flag, fixed recipient ATA, and sufficient protected base-fee balance in `withdraw_leftover.rs`. | Zero-floor mainnet profiles record zero leftover. Donations are not swept into treasury via this instruction. |
| Creator fee isolation | Authority and ledger PDAs include virtual pool; collection ATAs are canonical to that authority; receipt binds pool/lock/NFT/mints/vaults in `ix_claim_raydium_creator_fees.rs:14–87`. | Random pool/lock/ledger/vault/authority/account substitutions reject; direct permissionless external collection is counted later rather than stranded. |
| NFT-transfer authorization | Signer must own a token account holding exactly one of the receipt's fee NFTs; lock and pool bindings validated; destinations are owner's ATAs. | Existing actual-SBF tests cover unauthorized owner, redirected destination, invalid side, old holder rejection after transfer, and new holder claim. Delegate ownership alone is not accepted by the account constraint; no new delegated-account VM case was run. |
| Claim rounding / replay | Lifetime u128 paid ledger with checked arithmetic, current balance bound, and atomic recording/transfers (`state/raydium_creator_fees.rs:15–38`). | No per-call dust rounding advantage found. Existing unit tests cover repeated unit receipts, claim order, donations/external collection and replay. Existing SBF tests include both fee NFTs owned by one holder. |
| Vesting flash rent | Fixed locker ID/base PDA, official CPI, borrower-funded replenishment, invariant preserving pool authority lamports/owner/data size (`create_locker.rs`, `flash_rent.rs`, `utils/cpi_checker.rs`). | Reviewed statically only here. New mainnet configs reject vesting and bypass this legacy path. No claim of fresh adversarial SBF coverage for legacy nonzero vesting. |

## New executable test evidence

File: `<review-workspace>/output/matched-fee-contract-20261028/vm-tests/audit-graduation-20261029.test.ts`.

The file copies the existing offline harness setup into a separate test artifact, pins the exact Evolut SBF hash, and adds:

- 13 graduation substitutions: reimbursement, both external programs, CPMM authority/pool, LP source, locker authority, creator, creator lock, locker LP vault, partner, per-pool creator authority and permission account. Each asserts failure plus source pool/base vault/quote vault/mint rollback.
- 8 fee-claim substitutions: pool, lock, NFT token account, vault, authority, ledger, collected account and destination. Each asserts failure without collecting external fees or leaving a newly initialized legitimate ledger.
- Actual-SBF preservation of an unsolicited base-token donation through graduation and burn.
- Actual-SBF exact reduction of reimbursed rent by a 1-lamport prefund of the new CPMM pool.

Command: `EVOLUT_VM_NETWORK=mainnet node --import tsx --test audit-graduation-20261029.test.ts`, run in local Ubuntu/WSL from the test directory. Result: **23 passed, 0 failed, 0 skipped**; exit 0. The native bigint binding was unavailable and used its pure-JS fallback. Structured observation: `output/internal-security-audit-20261029/graduation-adversarial-results.json`.

These tests use actual SBF but synthetic VM accounts and test keys. Known fixed treasury/admin identities are substituted only in harness setup using the explicitly documented signature-verification bypass; attack claims and graduation execute with real test signatures. They neither prove live permission/config state nor include current real user accounts. Several negative tests use funded invalid keys, not another fully valid pool's accounts; the stronger cross-pool-binding argument also relies on code inspection and existing fee-authority isolation tests.

## Remaining coverage gaps

1. No independent external audit or exhaustive fuzz/property verification was performed. Successful finite test cases cannot prove absence of all exploits.
2. Raydium and locker code was exercised through pinned captured executables, not comprehensively re-audited. ABI, upgrade and governance changes need separate monitoring/revalidation.
3. No fresh live mainnet state read or on-chain claim/trade/graduation was sent in this sub-review. Parent review must connect the local hash and source evidence to the current deployed executable.
4. Legacy nonzero vesting/flash-rent paths received static review only; new mainnet configurations explicitly disable vesting. Claims under arbitrary historically corrupted state were not exhaustively fuzzed.
5. Tests do not prove total-system availability, RPC behavior, worker balance sufficiency, frontend quote/claim freshness or wallet UX. They establish contract-level behavior only for the covered fixtures.
6. The review did not add emergency authority to withdraw locked liquidity, bypass signatures, weaken matching-transaction checks, or change live fees. Any response to a blocked graduation requires its own concrete review rather than assuming this report authorizes a change.
