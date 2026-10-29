# Evolut internal security review — 29 October 2026

**Internal, AI-assisted review commissioned by the project owner. Not an independent external audit or certification.** This report records source inspection, offline adversarial execution and dependency triage of the reviewed contract and Jupiter adapter. It does not claim Jupiter acceptance or the absence of all vulnerabilities.

## Result in plain language

We did not establish an unauthorized fund-withdrawal, authority takeover, cross-pool fee theft, replay payout or LP-principal unlock exploit in the reviewed paths. **121 test cases passed across five suites**, including new negative cases against the actual captured mainnet executable. These suites share setup/code and are not 121 independent proofs.

There are still actionable gaps. Direct Classic pool creation does not require the website service fee. Fixed ticker enforcement depends on metadata authority mode. Graduation depends on Raydium continuing to support the chosen configuration and permission. Known dependency advisories and differing math dependency versions need coordinated remediation. The Jupiter adapter still requires Jupiter's router integration and CPI validation.

No deployed contract, live accounts, frontend, backend service or production configuration was changed. No mainnet transaction was sent. No inherited source or audit submission was published or sent to a third party. Advisory queries sent only public crate names and versions.

## Exact scope

- Mainnet program: `EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ`.
- Tested SBF: 1,207,880 bytes; SHA-256 `885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6`.
- Source: the 94 unchanged files in `vendor/evolut-curve`, verified against `vendor-source-manifest.json` and the 28 October owner release archive. Mainnet feature paths were reviewed explicitly.
- Adapter input revision: local repository commit `3e8e5082af9dff53d2e1b7792d8348c9d0d04e62`. This review adds tests/documentation; the final package manifest records their exact hashes.
- Jupiter interface/test kit: `70f56e0495b7c818e4c38e37b2007656892ca2a1`.
- The prior same-day finalized capture at slot 462865628 establishes the fixture/deployed executable match. The audit tests reuse that capture; this is not continuous monitoring or an independent reproducible-build attestation.
- Review areas: instruction dispatch; admin/operator/treasury separation; configuration and launch authorization; metadata; curve math and fee accounting; creator/partner/protocol claims and surplus; Raydium graduation, LP locks, burn and reimbursement; creator-fee claims; adapter account validation and quote/native parity.

Backend payment verification, website authentication, Railway access, operational secret custody, all third-party code, and live load/latency were **not** audited in this contract-focused engagement. Legacy nonzero vesting paths received static review but no fresh adversarial VM coverage. No new naming restrictions were added.

## Findings and decisions

| ID | Classification | Confirmed behavior and impact | Required follow-up |
| --- | --- | --- | --- |
| A-01 | Conditional business-control gap; no customer-fund theft shown | Classic configurations allow direct initialization without the website launch fee. Evolving requires the authorized worker/treasury, but its payment-to-launch binding is backend-managed. | Decide whether the fee purchases backend services or must be mandatory for all on-chain launches. If mandatory, implement atomic on-chain payment or a consumed payment authorization and separately review backend receipt binding. |
| A-02 | Informational metadata/disclosure issue | The operational-PDA Evolving update instruction preserves symbol and URI. Classic creators and treasury-controlled legacy metadata have broader Metaplex update powers. Upgrade authority is also trusted. | Qualify fixed-ticker statements; inspect each token's actual metadata authority. The Jupiter draft has been corrected. |
| GC-01 | Medium availability risk from a privileged external dependency | An official Raydium config/permission/ABI or rent change can prevent graduation after the curve completes. Fee/account checks correctly fail closed. | Monitor selected configuration, permission, external program upgrades and rent cap; prepare a specific incident/recovery procedure. Do not bypass fee/account checks. |
| GC-02 / GC-03 | Informational economic and external-program trust | Post-graduation creator fees split net receipts, after external deductions. LP principal is held by the official permanent locker; external program upgrade authorities remain trusted. | Describe net fees accurately and monitor official fee-policy/program changes. No absolute guarantee against future external upgrades is made. |
| MA-L02 / D-01 | Low demonstrated reachability; dependency remediation remains open | Contract uses `ruint` 1.14.0, adapter 1.17.2. Both match shift advisories; the older contract dependency also matches the reciprocal advisory. Relevant unsafe triggers were not found through supported current call paths. Other transitive advisory matches are recorded separately. | Prepare a coordinated patched dependency candidate, then rebuild/retest contract and adapter against old/new SBF before any owner-signed upgrade. Do not silently change only the quote engine. |
| MA-I01 | Integration work outstanding | `Swap::Placeholder` is a native-test seam. Real Jupiter CPI/multi-hop execution is not demonstrated. | Obtain Jupiter's routing variant/account mapping and run the actual integration path before describing the curve as Jupiter-routable. |

Severity reflects the observed project impact and preconditions, not merely an advisory's presence. The metadata and fee-enforcement observations are design/trust boundaries, not an accusation that arbitrary users can steal pooled funds.

## Test evidence

| Suite | Passed / failed | What it establishes for the tested fixtures |
| --- | --- | --- |
| Mainnet-feature Rust contract tests | 38 / 0 | 26 unit/property tests, four burn tests, three revenue-policy tests, five candidate-economics tests |
| Existing actual-SBF mainnet fee/graduation regression | 23 / 0 | 0.5/85 SOL paths, both tiers and mint orders, claims, legacy upgrade behavior, external-share changes, LP lock and late-failure rollback |
| Added authority/launch SBF tests | 10 / 0 | Unauthorized configuration/worker/claims rejected, payment bypass reproduced, metadata restrictions, payout rollback/replay and creator transfer |
| Added graduation/claim SBF tests | 23 / 0 | 13 graduation substitutions, eight claim substitutions, donation-preserving burn and exact prefunding reimbursement |
| Standalone Jupiter adapter suite | 27 / 0 | 17 validation, four execution, two parity, four added security-invariant tests |
| **Total** | **121 / 0** | No skipped test cases in these successful final runs |

The four new adapter tests include 113 native SBF swaps, 288 boundary quote probes and 28 independent fee-conservation vectors. They check repeated buy/sell round trips, integer extremes and a synthetic two-segment curve. They do not prove exhaustive coverage of all possible state/input combinations.

The official Jupiter kit covers six buys against an untouched live 2%/85 SOL snapshot. The 1% and two-segment states are explicitly synthetic. Sell tests perform a local buy first because the captured live pool contained one lamport of quote reserve. Finite vectors do not prove mathematical equivalence for every possible input.

VM tests use synthetic balances/configuration and generated user keys. Fixed admin/treasury identities are substituted only for documented offline setup/authorized-role scenarios; attack and ordinary user tests verify actual generated signatures. No real private keys are used. The external Raydium, locker and metadata programs are pinned captured binaries, not re-audited implementations.

## Dependency review

Public OSV advisory metadata was checked for **704 unique registry package/version pairs** across the contract and adapter lockfiles. It returned **22 package-version matches and 21 advisory identifiers**; some identifiers are aliases and some matches are maintenance notices or build/test dependencies. They are not 22 demonstrated vulnerabilities in the deployed program.

See `DEPENDENCY-REVIEW.md` and the saved advisory evidence for exact versions, primary references, target/dependency graph limitations and remediation. Git/path dependencies, npm tooling and a comprehensive transitive license audit are outside that scan. Known advisories were not suppressed to create a clean result.

## Reproduction and evidence locations

The standalone adapter is reproducible from `evolut-jupiter-amm` with Rust 1.94.1 and its locked dependencies:

```text
node scripts/verify-package.mjs
cargo test --locked --offline
cargo fmt --package evolut-jupiter-amm -- --check
```

Omit `--offline` on a first setup that needs public dependency downloads. Windows can use the included `run-cargo.mjs` helper. Do not format the unchanged vendor tree.

The broader contract suites use the existing local contract/operations VM harness and external fixtures. Their test sources, logs and exact commands are retained under `output/internal-security-audit-20261029` and `output/matched-fee-contract-20261028/vm-tests`. The standalone adapter repository includes review evidence but **does not bundle the full broader application-dependent VM environment**. It independently runs the 27 adapter cases, not all 121 cases from one command.

Supporting reports: [authorities/launch](internal-review/authorities-launch.md), [graduation/claims](internal-review/graduation-claims.md), [math/adapter](internal-review/math-adapter.md), [dependencies](internal-review/DEPENDENCY-REVIEW.md). Machine-readable evidence includes suite logs, advisory metadata, the package manifest and `AUDIT-EVIDENCE.json`. Raw local run logs may contain operator filesystem paths; packaged copies replace those paths and contain no wallet secrets.

## Limits, priorities and Jupiter submission

This was a bounded manual/AI-assisted review plus adversarial tests, not formal verification, an exhaustive fuzz campaign, a complete economic audit, an external audit, or a guarantee of production availability. No review can establish safety solely from a passing test count. Code upgrades, new authority modes, dependency changes and external program changes invalidate relevant assumptions and require re-review.

Recommended order: (1) clarify/close launch-payment requirements and correct metadata claims; (2) prepare dependency remediation with exact binary/quote parity; (3) implement configuration/permission monitoring and incident handling; (4) finish Jupiter CPI integration and remaining backend-payment review. None of these items is marked fixed merely by documenting it.

Jupiter's [integration guide](https://developers.jup.ag/docs/swap/routing/amm/integration) asks for “Your security audit”; it does not explicitly require an external auditor or publish acceptance criteria. This internal report can be presented honestly as internal. Ask whether it meets their review requirements; do not claim acceptance or an audit waiver.

Source rights are separate from this security result. GitHub permits viewing/forking public repositories, but that is not an established unrestricted commercial/SDK redistribution grant. The identified historical Meteora release had no located license grant; the later noncommercial license is not assumed retroactive. See `docs/SOURCE-RIGHTS.md` for evidence. No new license was invented and no request was sent externally.
