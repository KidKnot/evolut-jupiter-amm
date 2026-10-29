# Internal review remediation addendum — 29 October 2026

Internal, AI-assisted work. Not an independent audit or certification. This addendum records remediation tested locally before upgrade. The owner-signed upgrade was subsequently verified against the same executable; see [release status](RELEASE-STATUS.md) and [upgrade verification](../evidence/mainnet-upgrade-verification-20261029.json). Matching backend rollout remains pending. These later status updates do not relabel the historical tests or account captures.

## Distinct reviewed releases

| Identity | Recorded deployed baseline | Remediation candidate, subsequently matched on mainnet |
| --- | --- | --- |
| Program | `EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ` | Same program ID |
| Executable bytes | 1,207,880 | 1,142,312 |
| SHA-256 | `885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6` | `e9e70baeb58c545dae66b86f67c5ff44a79a77dc06a44a9a69654ae3a3dcf441` |
| Application release | Historical running release | Prepared commit `9f4c6212146f0bf98c35057a5b2dfc7213d1cc30`; rollout pending |

The original report describes the baseline and remains preserved. Its 121 cases are not relabeled as tests of the candidate.

## Clarification and changes

The original A-01 finding was conditional on whether on-chain fee enforcement was desired. Further application inspection established that Classic launches had **no platform launch charge** at baseline; it was not a bypass of an existing Classic fee. Evolving’s 0.015 SOL payment was bound by backend processing. The owner then explicitly approved a mandatory 0.015 SOL charge for both products.

The upgraded program transfers exactly 15,000,000 lamports to the canonical treasury inside every new mainnet pool initialization, including using old configs. Missing or substituted recipients fail. Failure of creation or a later initial buy in the same transaction rolls back the platform transfer. Existing pools, trading/claim ABI and devnet fee behavior are unchanged. Only error 6058 is appended to the IDL.

The prepared matching backend preserves historical signed receipts and prevents stale fee-free Classic approvals from silently gaining a charge. Its compatibility path uses operator-funded credit for previously paid split-treasury Evolving jobs completed after upgrade, avoiding a second customer charge. New Evolving jobs still require a separate upfront funding transaction for AI/creation. It is not escrow or an automatic-refund mechanism. Deployment and live verification of this backend remain separate outstanding work.

Compatible dependencies were updated, including the same exact `ruint 1.20.0` in execution and quoting. Candidate build uses official Anza platform-tools v1.58 with explicit SBPF v0. Two builds produced the same hash. Maintenance and feature/reachability-qualified advisory matches remain documented; no blanket advisory suppression or clean-bill-of-health claim is made.

## Candidate verification

- 39 mainnet host cases and 37 default/devnet cases passed.
- 62 actual-SBF cases passed: mandatory launch fees, recipient substitutions, exact-once retry, rollback, existing-pool preservation, trading, claims and graduation.
- 120 application cases passed; 26 affected cases were rerun after review and overlap that suite. Both typechecks and production builds passed.
- 28 upgrade-tool cases plus the full offline native-loader upload/upgrade/refund test passed.
- Five focused dependency regressions passed.
- 27 adapter cases passed against the captured baseline, then 27 against the candidate. The historical account capture was not mislabeled as a new deployed capture.

Remaining boundaries include trusted upgrade/metadata authorities, external Raydium configuration/permission dependencies, a limited live-pool fixture set, and unverified final Jupiter CPI/multi-hop integration. Inherited-source rights and Jupiter’s acceptance of internal review are unresolved. See the [original report](INTERNAL-SECURITY-REVIEW-20261029.md), [dependency remediation](DEPENDENCY-REMEDIATION.md) and [source-rights disclosure](SOURCE-RIGHTS.md) for their complete limits.
