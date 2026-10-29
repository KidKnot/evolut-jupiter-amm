# Release status — 29 October 2026

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

The matching application rollout is being verified separately. This repository
does not assert that the website/backend rollout has passed acceptance.

The recorded account snapshots and local test logs retain their original dates
and identities. They have not been relabeled as new live account captures or
on-chain trade tests. The program fixture was built locally and now matches the
verified mainnet executable.

Jupiter submission, routing activation and review acceptance remain pending.
The review is internal, not an independent audit. [Source rights](SOURCE-RIGHTS.md)
remain unresolved; public availability does not establish a permissive license.
