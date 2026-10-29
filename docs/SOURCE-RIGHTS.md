# Source rights and provenance

Checked 29 October 2026. **The owner has explicitly authorized public publication
of this integration package. Inherited-source permission remains unresolved:**
the owner confirmed that no separate permission or original license grant is
available. Publication is not a claim of a permissive license or cleared
commercial-use rights.

The earlier baseline preserved 94 files from the reviewed Evolut release source
archive byte-for-byte. This separate atomic-launch candidate has 95 files;
`vendor-source-manifest.json` identifies every change and preserves the baseline
manifest in `evidence/atomic-launch`. Source changes do not resolve or grant
rights to the inherited code. The lineage comparison below records the earlier
baseline investigation, not a new candidate-wide similarity calculation.

## What was traced

| Evidence | Finding |
| --- | --- |
| Candidate upstream lineage | Meteora DBC [Release 0.1.8, 16 January 2026](https://github.com/MeteoraAg/dynamic-bonding-curve/commit/828f350831a0f3c085b64c4998a27ccf7d84a76a) |
| Original supplied workspace comparison | 53 of 75 selected files match: three byte-exact and 50 after CRLF normalization |
| Current reviewed vendor comparison | 40 of 79 selected files match that release after CRLF normalization; other files include Evolut modifications |
| Exact acquisition revision/terms | Not established: the original supplied workspace had no Git metadata or license grant |
| Upstream license addition | [6 February 2026, commit 34bfbe06](https://github.com/MeteoraAg/dynamic-bonding-curve/commit/34bfbe06fe9a33cb55f5bd4a12b618903a5a5f27) |
| Current checked license | Non-commercial Licence; unchanged since its introduction |
| Earlier MIT/Apache or other permissive grant | None found in the historical repository tree/manifests/license history reviewed |

See [history evidence](../evidence/upstream-license-history.json) and
[per-file lineage comparison](../evidence/upstream-file-comparison.json).
File similarity establishes lineage, not the exact acquisition terms.

The later noncommercial terms are not asserted to govern older source
retroactively. An earlier missing license also does not establish commercial
redistribution rights. [GitHub's licensing explanation](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository)
distinguishes default copyright protection from the platform's view/fork rights.

The owner's observation that public repositories may be forked is correct within
GitHub's platform terms. That does not by itself establish an unrestricted grant
for commercial deployment or a redistributed SDK that Jupiter may operate and
maintain. This report does not deny GitHub's built-in fork permission or assert
that the later license retroactively changes earlier terms.

## Unresolved terms

An applicable original grant or written rightsholder permission is still needed
to establish the terms covering:

1. Evolut's modified, fee-earning mainnet deployment of the inherited source.
2. Distribution of the contract math/source and quote adapter for this integration.
3. Jupiter's ability to fork, modify, maintain and operate the SDK, including the
   required notices and any restrictions.

A private upload or a new MIT/Apache label does not resolve those rights. Raydium
pool-creation permission is unrelated to Meteora-derived source licensing.
The [permission inquiry](PERMISSION-REQUEST.md) is prepared but has not been sent.

## Preserved notices

The reviewed source retains its [provenance notice](../vendor/evolut-curve/UPSTREAM-NOTICES.md)
and attribution comments. Some math comments reference Solana SPL and
`integer-sqrt-rs`; applicable third-party notices are preserved. Their permissive
licenses do not license the surrounding Meteora code.
This investigation is not a complete transitive-dependency license audit or a
legal determination about any separate agreement.

No license file was invented or applied to inherited code. The Cargo package
remains `publish = false`; this setting itself is not a legal restriction or grant.
