# Atomic launch and patched dependency candidate

Prepared 29 October 2026. The tests below were performed on a **local candidate**. A subsequent finalized mainnet check matched its exact executable hash and length after the owner-signed upgrade; see [release status](RELEASE-STATUS.md) and [upgrade verification](../evidence/mainnet-upgrade-verification-20261029.json). The compatible backend is now live; its separate release/quote and admission checks are recorded in [application verification](../evidence/application-rollout-verification-20261029.json). The earlier internal review remains historical baseline evidence and is not relabeled as a review of this changed binary.

This candidate adds an atomic 0.015 SOL treasury fee to Classic and Evolving initialization. It also updates compatible dependency versions, including the same exact patched `ruint 1.20.0` in SBF and adapter. Trading fee percentages, existing pool account layouts and quote semantics are unchanged by these edits. Launch-transaction enforcement is tested in the separate contract VM suite, not inferred from swap parity.

## Two separately recorded adapter runs

| Run | Program fixture | Results |
| --- | --- | --- |
| Patched adapter/dependencies against captured baseline | 1,207,880 bytes; SHA-256 `885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6` | **27 passed, 0 failed** |
| Patched adapter/dependencies against new local candidate | 1,142,312 bytes; SHA-256 `e9e70baeb58c545dae66b86f67c5ff44a79a77dc06a44a9a69654ae3a3dcf441` | **27 passed, 0 failed** |

Both runs used `node run-cargo.mjs test --locked --offline --all-targets --jobs 2`, Rust 1.94.1/MSVC. Formatting also passed. The first default-parallelism compile hit a host compiler access violation; the bounded two-job retry succeeded with no source/dependency workaround.

Each run comprises 17 validation tests, four execution tests, two parity tests and four security-invariant tests. The official Jupiter kit executes six buy cases, including the exact remaining 85 SOL threshold. Supplemental native tests cover sells, fee rounding/buckets, slippage rollback, repeated round trips, integer boundaries and synthetic segment transitions. These are local executions, not mainnet transactions or proof of Jupiter CPI/multi-hop integration.

Logs are in `evidence/atomic-launch/`. `evidence/validation.json` identifies both runs; `tests/fixtures/program-under-test.json` binds the current executable. The captured account JSON and its recorded mainnet program hash were **not rewritten**: that hash still describes the historical capture. `tests/fixtures/evolut.so` now contains the separately identified local candidate. The baseline executable is retained under `evidence/atomic-launch/captured-baseline-885d9c88.so`.

## Source and build evidence

`vendor-source-manifest.json` lists 95 files and identifies the nine changed/added files relative to the preserved 94-file baseline manifest. The new manifest does not claim unchanged or deployed source. It includes the final candidate contract lockfile; the adapter's own lockfile pins its separate host dependency resolution.

The candidate SBF was built twice with official Anza platform-tools v1.58/Rust 1.95.0-dev using explicit **SBPF v0**. Both builds produced the same hash above. ELF flags are 0. The old installed compiler and its `solana` alias were preserved. The subsequent live executable/authority check is recorded separately from this build and test evidence.

See [dependency remediation](DEPENDENCY-REMEDIATION.md) for versions, five targeted release regressions, remaining advisory matches and compiler archive provenance. The raw candidate advisory scan is preserved. No blanket advisory ignore list or replacement license was added.

## Still separate

The owner-signed upgrade and compatible application rollout were verified separately after these local adapter tests. Source-rights and Jupiter routing/audit-acceptance questions remain as documented. No new live pool capture, trade, claim, graduation or external submission is established by these tests. The later backend checks sent no financial transaction and do not prove a paid launch or nonzero payout. The [remediation addendum](REMEDIATION-ADDENDUM.md) also corrects the historical Classic-fee interpretation.
