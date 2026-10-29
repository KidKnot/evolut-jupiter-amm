# Candidate dependency remediation — 29 October 2026

This evidence concerns only `output/atomic-launch-contract-20261029` and `output/atomic-launch-jupiter-20261029`. It does not change or certify the currently deployed program, the prior audit fixture, or a live service. The parent release review supplies transaction, ABI, economics and VM checks for the final artifact.

## Compatible updates

| Dependency | Contract candidate | Adapter candidate | Reason |
| --- | --- | --- | --- |
| ruint | 1.14.0 → **exactly 1.20.0** | 1.17.2 → **exactly 1.20.0** | Fix reciprocal/shift advisories and remove differing arithmetic versions between execution and quoting. Both root manifests and the adapter's vendored contract manifest pin this version. |
| borsh 0.10 branch | 0.10.3 → 0.10.4 | Already 0.10.4 | Patched non-Copy zero-sized-type deserialization; preserve Anchor/Solana serialization compatibility. |
| anyhow | 1.0.97 → 1.0.104 | Already 1.0.104 | Mutable-downcast aliasing fix. |
| bytes | 1.10.1 → 1.11.1 | 1.11.0 → 1.11.1 | Reserve/overflow fix, including test/tool graph. |
| keccak | 0.1.4 → 0.1.6 | Already patched | ARMv8 assembly fix; the SBF target itself is not ARMv8. |
| rand 0.8 branch | 0.8.5 → 0.8.6 | 0.8.5 → 0.8.6 | Reentrant custom logging/RNG fix. |
| rand 0.9 branch | 0.9.2 → 0.9.3 | Not present | Same fix in its compatible branch. |
| ctutils | Not present | 0.4.2 → 0.4.4 | Constant-time operation fix. |

Updates were targeted `cargo update -p ... --precise ...` operations, not an unconstrained ecosystem upgrade. Resolver changes required by ruint 1.20 are recorded in the lockfiles. Host Rust is pinned to 1.94.1 for the contract; the adapter's minimum Rust is 1.90 because ruint 1.20 requires it. No locally modified crypto implementation or blanket advisory ignore list was introduced.

## Focused security regressions

`node run-cargo.mjs test --release --locked --offline` in this directory: **5 passed, 0 failed**. See `tests/advisories.rs` and `dependency-regression-tests.log`.

- Fixed shift handling: discarded whole limbs, a non-word-sized integer, and a shift count greater than 2³².
- Safe reciprocal input rejection and division quotient/remainder conservation near normalization boundaries.
- Three callback-panic/reuse checks against the actual pinned block-buffer 0.9/0.10 implementations.

These are ordinary release-mode tests plus source inspection, not a Miri proof or an independent security audit.

## Remaining scanner matches, without concealment

The new OSV query covered **733 unique registry package versions** and returned **12 package-version matches / 11 advisory IDs** (aliases overlap), versus 22/21 in the original scan. `dependency-advisories.json` retains the full response, timestamp and exact lockfile hashes. The query transmitted public package names/versions only. Remaining findings are not silently suppressed:

| Match | Evidence and disposition |
| --- | --- |
| borsh 0.10.4 / GHSA-fjx5-qpf4-xjf2 | The GHSA range still matches, but primary RustSec RUSTSEC-2023-0033 lists `^0.10.4` as patched. Both candidates now use that patched release. |
| block-buffer 0.9.0 and 0.10.4 / GHSA-qwgh-2vcv-g2f7 | The advisory describes an **inline cursor overwritten by a callback**. Both pinned source versions instead hold `pos` as a separate struct field (`usize` in 0.9, `u8` in 0.10); callbacks cannot overwrite it through the block. The newer `ReadBuffer` type in the advisory is not present. Regressions recreate the published callback-panic pattern on these versions and pass. This corrects the earlier audit's broad range-only triage; it is not a claim that all buffer APIs or all versions are free of bugs. |
| rand 0.7.3 / RUSTSEC-2026-0097 and alias | No patched 0.7 branch is published. Its required optional **log feature is disabled** in the candidate adapter graph; it is absent from the candidate SBF runtime graph. `adapter-rand07-features.log` records enabled features. A downstream consumer that unifies/enables `rand/log` must reassess this finding; the adapter itself does not install a custom RNG-using logger. |
| crypto-bigint 0.5.5 / RUSTSEC-2026-0387 | Retained transitively in the adapter lockfile but absent from the selected normal runtime dependency graph. The fixed 0.7 branch requires a coordinated upstream crypto dependency migration, not a compatible direct override. Do not enable new secret-bearing uses without that migration/review. |
| rkyv 0.7.46 / RUSTSEC-2026-0235 | Retained optional/test dependency, absent from selected normal graphs. The 0.8.17+ fix changes the archive API/format; no application archive-parser use is enabled here. Do not enable untrusted rkyv archive parsing against the old branch. |
| bincode 1.3.3, derivative 2.2.0, libsecp256k1 0.6.0/0.7.2, paste 1.0.15, ansi_term 0.12.1 | Unmaintained-package notices, not by themselves a demonstrated exploit. Some are Solana/Anchor runtime or macro dependencies, others tooling. Replacing serialization/cryptography with incompatible majors solely to remove a notice is outside this compatible release; track upstream maintenance. |

`contract-runtime-dependencies.log` and `adapter-runtime-dependencies.log` were captured using `cargo tree --locked --offline --edges normal` for SBPF v0 and the Windows host respectively. Macro dependencies can appear in these graphs without being linked into the SBF. “Absent” here is a statement about the selected graph, not a guarantee for arbitrary downstream feature combinations.

## SBF compiler

The old installed SBF compiler is Rust 1.79, which cannot compile patched ruint 1.20 (edition 2024, MSRV 1.90). The candidate therefore uses the official **Anza platform-tools v1.58, Rust 1.95.0-dev**, with official `cargo-build-sbf 4.4.0`. The wrapper uses **`--arch v0 --no-rustup-override --skip-tools-install --tools-version v1.58`**; the default v3 target is deliberately not used. No installed `solana` rustup alias is removed or changed.

Official Linux x86_64 platform archive: 531,406,444 bytes. SHA-256, independently compared with the GitHub release asset digest:

`7b82eb420b5aebd1e5590db0ae25c523eca06724d4238ae02139690535636c37`

The reproducible Windows build command is `node build.mjs sbf-wsl --features mainnet` from the candidate contract directory. A first equivalent command is recorded in `output/atomic-launch-toolchain-20261029/build-candidate-sbf.sh`. Fixed paths point only to the isolated candidate and separately installed compiler. Both builds succeeded and produced the same artifact: **1,142,312 bytes**, SHA-256 **`e9e70baeb58c545dae66b86f67c5ff44a79a77dc06a44a9a69654ae3a3dcf441`**, ELF64/Solana machine 263, **ELF flags 0 (SBPF v0)**. The two logs are `candidate-sbf-build.log` and `reproducible-helper-check.log`; `candidate-sbf-artifact.json` and `toolchain-and-elf-proof.log` record the binary and preserved original compiler alias.

Official wrapper 4.5.0 required a newer host compiler than installed; official 4.4.0 was selected instead. Its first parallel install encountered a host rustc SIGSEGV; a two-job retry completed successfully. Neither attempt changed the program or any deployed service.

## Primary references

- https://rustsec.org/advisories/RUSTSEC-2025-0137.html
- https://rustsec.org/advisories/RUSTSEC-2026-0220.html
- https://rustsec.org/advisories/RUSTSEC-2023-0033.html
- https://rustsec.org/advisories/RUSTSEC-2026-0190.html
- https://rustsec.org/advisories/RUSTSEC-2026-0007.html
- https://rustsec.org/advisories/RUSTSEC-2026-0097.html
- https://github.com/RustCrypto/utils/security/advisories/GHSA-qwgh-2vcv-g2f7
- https://github.com/anza-xyz/platform-tools/releases/tag/v1.58
- https://github.com/anza-xyz/cargo-build-sbf

This remediation must ship as one reviewed contract/adapter release. Updating a lockfile alone cannot patch the already-deployed SBF; final owner-signed upgrade and consumer rollouts remain separate steps.
