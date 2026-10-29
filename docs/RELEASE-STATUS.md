# Release status — 29 October 2026

The [SDK repository](https://github.com/KidKnot/evolut-jupiter-amm) was published
on 29 October 2026. Repository and review-document access were verified without
authentication. This is source publication, not a Jupiter submission or approval.

The initial public commit `403c2a10f62adca51dae8d22a1743333d6b54f16` passed
[GitHub CI](https://github.com/KidKnot/evolut-jupiter-amm/actions/runs/40893923842):
package/source integrity, formatting, and the adapter test suite on Ubuntu.
This is additional reproduction of the tests, not an independent security audit.

The owner-signed mainnet upgrade is **finalized**. At 14:35:14 UTC, a bounded,
read-only check verified the executable's bytes and SHA-256 against the exact
fixture used by this repository's 27 passing adapter tests.

| Item | Verified value |
| --- | --- |
| Program | `EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ` |
| Deployment slot | `462914292` |
| Finalized account-read slot | `462916600` |
| Executable bytes | `1,142,312` |
| Executable SHA-256 | `e9e70baeb58c545dae66b86f67c5ff44a79a77dc06a44a9a69654ae3a3dcf441` |
| Upgrade authority | `Ddu3yPxCZpfgc3h3cKTkbpv2W7WyNhc8eDf7gGZd6whY` |

[Upgrade transaction](https://explorer.solana.com/tx/33Li3HT6YHgvdsiZk6ABvjfnQJPDw3TPKUh5qhaAQhA6rpow5cC9CG1BAbxH8g7BnPSCSg1CP5b2X75tfiUpCix8)
was independently checked as finalized and successful at the same deployment
slot. [Machine-readable verification](../evidence/mainnet-upgrade-verification-20261029.json)
contains only public evidence. This verification submitted no transaction.

The new program transfers 0.015 SOL to the canonical treasury as part of each new
mainnet Classic or Evolving pool initialization. Existing-pool swap and claim
interfaces remain unchanged. The [remediation addendum](REMEDIATION-ADDENDUM.md)
explains the change and corrects the earlier review's interpretation of Classic
launches, which were free at the historical baseline.

The compatible application release `0324f9cc4356f9b1e8e2a368a06526a7f7fa1675`
is live. The final API check matched all 440 deployed file hashes and all four
service roles were healthy with matching release artifacts. Acceptance checks
at 14:52:41 UTC covered Classic/Evolving with both 1%/2% configurations, the
85 SOL graduation threshold, the 50% creator share, 15,000,000-lamport launch
quotes, API/worker capabilities and a buffer of 20 available mint addresses.
At 14:55:56 UTC, public launches, paid jobs and atomic launch fees were verified
enabled. See [application rollout verification](../evidence/application-rollout-verification-20261029.json).

Curve-fee account reads and unsigned Raydium LP/creator-claim simulations also
passed. All sampled claim balances were zero: these checks do **not** verify a
nonzero payout. No financial transaction or paid launch was submitted by this
verification, and it is not a claim of a new live graduation test.

The recorded account snapshots and local test logs retain their original dates
and identities. They have not been relabeled as new live account captures or
on-chain trade tests. The program fixture was built locally and now matches the
verified mainnet executable.

Jupiter submission, routing activation and review acceptance remain pending.
The review is internal, not an independent audit. [Source rights](SOURCE-RIGHTS.md)
remain unresolved; public availability does not establish a permissive license.
