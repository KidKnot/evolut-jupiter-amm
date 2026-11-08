# Combined mainnet upgrade candidate — 8 November 2026

This source candidate adds Irys metadata, native-SOL fee claims and milestone buybacks. **The owner upgrade and activation are pending.** Source commit: `4257bde5acbf63db0245ae442dad5545f26232f8`.

| Artifact | Value |
| --- | --- |
| Candidate executable SHA-256 | `912f2209678bc42de04d2127745cb2321d0c828666a31a6b2cc527f987c77796` |
| Candidate executable bytes | 1437320 |
| Mainnet IDL SHA-256 | `0ee523f9105b07ab85fc901a1ed6454e1d9def8d75894548867a08a88f0da142` |
| Full-workspace local tests | 44 Rust; 105 actual-SBF cases |

The Jupiter adapter's curve quote math and swap ABI are unchanged. Existing 29 instructions, 7 account layouts and 51 types keep their wire representation. The generated IDL adds 11 buyback instructions, 2 accounts, 4 types and one event. The ABI check ignores documentation text only.

**Graduation caller change:** on mainnet, append the canonical `buyback_policy` PDA after the prior route-specific remaining accounts. It is required even while the policy is absent or disabled. An enabled policy routes new Evolving platform FeeKeys to the per-pool program vault; disabled or uninitialized policy preserves treasury custody. The fixed graduation account list and instruction data are unchanged. Applications must select the correct remaining-account layout from the verified executable; IDL compatibility alone is insufficient.

Only Evolut's actually collected net Raydium fee share on participating Evolving coins is allocated: 80% treasury, 10% source-token reserve and 10% platform-token reserve. Each confirmed evolution from $250k through stage 8 freezes a separate budget. Creator income, curve/launch/graduation fees and locked LP principal are excluded. After the last milestone, newly collected fees go entirely to treasury; earlier frozen budgets remain executable. The platform leg waits if its mint is unset.

The authorized worker attests the evolution milestone; this is not a trustless USD oracle. The contract enforces custody, nonces, pool/mint binding, integer conservation, bounded execution and atomic buy-and-burn. Existing coins require treasury-signed adoption. Cold admin can pause and recover unspent reserves only to the fixed treasury; recovery does not release LP principal or the FeeKey. This makes unspent reserves admin-recoverable, not permanently locked. A failed buyback does not block evolution.

The standalone repository keeps its existing deployed-baseline executable and account fixtures. Run `node scripts/verify-package.mjs`, `node scripts/verify-upgrade-abi.mjs` and the adapter tests. Full-candidate SBF tests were run in the application workspace; no new executable or secret is included here. This is internal testing, not independent audit approval or Jupiter routing activation.

[Candidate evidence](../evidence/buybacks-20261108/candidate.json) · [Source manifest](../vendor-source-manifest.json) · [Source rights](SOURCE-RIGHTS.md)
