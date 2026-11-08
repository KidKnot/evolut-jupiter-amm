# Irys metadata and native SOL claims candidate — 8 November 2026

This earlier candidate is superseded by the [combined buyback upgrade](BUYBACK-CANDIDATE.md). The evidence below remains historical.

The vendored contract source and mainnet IDL now include the tested upgrade candidate. **It is not deployed.** The program ID, upgrade authority, existing swap instructions, fee split and account layouts are unchanged.

| Artifact | SHA-256 |
| --- | --- |
| Verified deployed baseline and existing test executable | `e9e70baeb58c545dae66b86f67c5ff44a79a77dc06a44a9a69654ae3a3dcf441` |
| Prepared upgrade, 1,323,464 bytes | `d6b91ac802247b85bf790879e3b6c0506c40e8665e80da4cd116d18d634d425a` |
| Prepared mainnet IDL | `4ecc5346abe23fcd3bd238e82a934042a3a57126af99c184a7f65c94e71dfddc` |

Added instructions:

- `update_operational_metadata_uri`: update a token's name and verified Irys metadata URI together; ticker and authorities remain unchanged.
- `claim_curve_fees_sol`: return supported accrued curve quote fees as native SOL.
- `claim_raydium_lp_fees_sol`: collect earned locked-LP fees, convert only the newly collected token amount through the bound pool, then unwrap SOL.
- `claim_raydium_creator_fees_sol`: settle the existing creator-fee entitlement through the same native-SOL route.

Claims enforce owner rights, fresh temporary accounts and minimum output. A failed conversion rolls back the entire claim. Existing wallet balances and locked liquidity principal are preserved. The original claim instructions remain available for compatibility. This change does not implement a buyback policy.

The IDL preserves all 25 old instructions, 7 accounts and 50 types, and adds one payout event. Run `node scripts/verify-upgrade-abi.mjs` to reproduce the ABI comparison against the recorded deployed baseline.

The full application workspace passed 40 mainnet Rust tests, 83 actual-SBF VM tests and a complete loader upload/upgrade/refund rehearsal. Those are local tests, not mainnet transactions or an independent security audit. This standalone repository's adapter tests still execute against the unchanged deployed-baseline fixture; no new binary or account capture was substituted. All 27 standalone adapter tests and the formatting check passed offline on 8 November. Its curve quote math and swap behavior are unchanged. The owner upgrade and post-upgrade live checks remain pending.

[Candidate evidence](../evidence/irys-sol-20261108/candidate.json) records hashes and test scope. [Source provenance](../vendor-source-manifest.json) lists the five updated previously published files and one new Evolut SOL-settlement helper. Existing notices and source-rights documentation remain intact.
