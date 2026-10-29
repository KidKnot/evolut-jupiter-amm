# Evolut Jupiter AMM adapter

Rust integration package for quoting Evolut's pre-graduation Solana bonding
curves through Jupiter Metis. Jupiter routing is **not enabled** by this package;
their review and router-side integration remain outstanding.

**Integration review package.** The owner authorized public publication;
inherited-source permissions remain unresolved as disclosed in
[source rights](docs/SOURCE-RIGHTS.md). No permissive license is asserted.
`publish = false` disables crates.io publication. No independent external audit
of the current Evolut program is available. The
[internal security review](docs/INTERNAL-SECURITY-REVIEW-20261029.md) records
findings, limitations and 121 passing cases for the **previous deployed baseline**.
The atomic-launch/dependency executable in this checkout was subsequently
verified on mainnet after the owner-signed upgrade. Matching backend rollout is
still pending. See [release status](docs/RELEASE-STATUS.md), the
[remediation addendum](docs/REMEDIATION-ADDENDUM.md), and
[candidate validation](docs/ATOMIC-LAUNCH-CANDIDATE.md). Historical test results
remain identified by the binary and account capture used at test time.

## Program and scope

Mainnet program:
[`EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ`](https://solscan.io/account/EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ).

- Legacy SPL Token against wrapped SOL; flat 1% / 2% curve fees.
- Full-consumption ExactIn buys and sells, with integer rounding from the
  reviewed contract math and a 50/50 creator/partner split. Arithmetic is pinned
  to patched `ruint =1.20.0` in both the candidate SBF and adapter.
- Each pool's own immutable config determines its curve and graduation threshold.
  New production configs use 85 SOL; historical test pools retain their settings.
- Finished curves are excluded. Graduated pools use official Raydium CPMM and
  need its existing integration, not this curve adapter.
- No networking, signing, transaction submission, wallet keys, account creation
  or SOL wrapping inside the adapter.

ExactOut, partial fills, referrals, Token-2022/extensions, dynamic/scheduled/
rate-limited fees and unsupported configuration shapes fail explicitly.

## Reproduce the evidence

Install Rust 1.90 or newer plus the platform's C/C++ build toolchain. Validation
used stable Rust 1.94.1. From this repository:

```sh
node scripts/verify-package.mjs
cargo test --locked
```

Node is only needed for the evidence verifier and optional capture tooling; the
Rust tests themselves use committed fixtures and no RPC access. After dependencies
are cached, use `cargo test --locked --offline`.

On Windows, open an x64 Visual Studio developer terminal, or with Visual Studio
2022 Build Tools installed use:

```powershell
node run-cargo.mjs test --locked --offline
```

The helper uses the installed stable Rust toolchain and prepares the MSVC
environment only when needed. It never deploys. Preserve `Cargo.lock`: the
Jupiter interface and official test kit are pinned to the same revision,
`70f56e0495b7c818e4c38e37b2007656892ca2a1`, including compatible sysvars.

## What was tested

The adapter has **27 tests**: 17 validation tests, four execution tests,
two parity tests and four additional security-invariant tests. One parity test includes six official-kit swaps, including
the exact remaining graduation threshold; the other verifies the program hash.

- The official kit executes six buys against an **untouched finalized 2% / 85 SOL
  pool snapshot**. The historical run used the captured deployed executable;
  the second run used the locally built candidate executable, subsequently
  matched to the upgraded mainnet executable by a separate read-only check.
- Additional LiteSVM executions buy and then sell, verifying input/output,
  vault/reserve changes, fee rounding and beneficiary buckets, slippage rollback,
  and rejection of threshold-crossing partial fills.
- The 1% execution variant is **synthetic**, made by changing the fee locally.
  It is not evidence of an unmodified live 1% pool. Sell tests first buy locally
  because the captured live pool had only one lamport of quote reserve.
- Validation rejects malformed, missing or stale account state, invalid
  identities, unsupported economics, inactive and completed pools.
- New adversarial cases execute 113 native swaps, probe 288 integer-boundary
  quotes and check 28 fee-conservation vectors, including synthetic segment crossing.

These are native top-level execution tests. Final Jupiter CPI/multi-hop mapping,
their routing variant and market eligibility still require review. Internal
tests and upstream audits are not an independent audit of Evolut's changes.
The broader internal report also uses the existing application-dependent VM
harness; this repository alone reproduces the 27 adapter cases, not all 121.

## Files a reviewer needs

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Jupiter interface, account validation/cache and native account metas |
| `src/quote.rs` | Candidate curve math and fee/output conversion |
| `tests/` | Official-kit parity, supplemental execution and rejection tests |
| `tests/fixtures/` | Public finalized account snapshots and explicitly identified candidate SBF binary |
| `vendor/evolut-curve/` | 95 candidate source files, including the mandatory launch-fee change |
| `vendor-source-manifest.json` | Exact candidate hashes and changes from the preserved baseline manifest |
| `package-manifest.json` | Exact hashes of the packaged inputs and evidence |
| `evidence/validation.json` | Test environment, results and limits |
| [Live pools](docs/LIVE-POOLS.md) | Representative active curve and graduated LP-lock proof |
| [Submission](docs/JUPITER-SUBMISSION.md) | Prepared form text; not submitted |
| [Internal review](docs/INTERNAL-SECURITY-REVIEW-20261029.md) | Findings, scope, test evidence and open dependencies |
| [Remediation addendum](docs/REMEDIATION-ADDENDUM.md) | Classic fee clarification, subsequent changes and their tests |
| [Release status](docs/RELEASE-STATUS.md) | Verified mainnet executable and remaining rollout work |
| [Source rights](docs/SOURCE-RIGHTS.md) | Traced provenance and remaining permission question |

`Swap::Placeholder` intentionally marks the routing seam awaiting Jupiter's
assigned variant. Arbitrary valid router token accounts are supported; the
transfer authority may differ from the user/payer. No deployment or backend
secrets are required to build or test this repository.

## Refresh public fixtures deliberately

The saved tests need no Node dependencies or live RPC. To deliberately recapture
fixtures, install optional tooling with `npm ci --ignore-scripts`, configure
`EVOLUT_FIXTURE_RPC_URL` in your environment if the public endpoint is inadequate,
then run `node capture-mainnet.mjs` **in a separate baseline capture checkout**.
Do not overwrite the candidate executable/identity with a live capture. Never
commit an endpoint credential.

The tool allows only bounded read RPCs and checks mainnet genesis and the pinned
program hash. Review fixture changes and test identities before accepting them;
it may select a different pool as markets graduate. Recapturing does not update
the evidence manifest or imply parity has passed on the new snapshot.

## Contact and review

- Project: Evolut — `info@evolut.fun`, [@evolutdotfun](https://x.com/evolutdotfun).
- [Jupiter integration requirements](https://developers.jup.ag/docs/swap/routing/amm/integration).
- [Jupiter AMM submission form](https://support.jup.ag/requests/new/amm-integrators).

Inherited-source terms, Jupiter's acceptance of the internal report or any
additional audit requirement, and Jupiter approval
remain separate from passing local tests. This package asserts no partnership,
verification badge or routing activation.
