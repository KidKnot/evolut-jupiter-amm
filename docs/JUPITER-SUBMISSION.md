# Jupiter bonding-curve integration submission

Prepared 29 October 2026; not sent. This is an initial technical inquiry, not a
claim that all acceptance requirements are complete. No fake repository link,
independent audit, funding, endorsement or naming safeguards are claimed.

**Release update:** the owner-signed mainnet upgrade has been verified against
the atomic-launch/dependency executable in this checkout. The compatible backend
is live and its release/quote and admission checks passed. See [release status](RELEASE-STATUS.md),
[candidate validation](ATOMIC-LAUNCH-CANDIDATE.md) and the
[remediation addendum](REMEDIATION-ADDENDUM.md). The SDK is now public at
https://github.com/KidKnot/evolut-jupiter-amm and its links work without a login.
The description below includes public URLs for the form.

Form: https://support.jup.ag/requests/new/amm-integrators

| Field | Value |
| --- | --- |
| Email | info@evolut.fun |
| Project | Evolut |
| Telegram | Keep the username already entered by the owner |
| Integration type | Bonding Curve |
| Subject | Evolut bonding-curve integration — adapter, internal security review and mainnet examples |

## Description — copy into the form

Hi Jupiter team,

I'm Kid Knot, founder of Evolut, a self-funded Solana launch platform with Classic and Evolving coins. We would like a technical review for integrating our pre-graduation bonding-curve markets into Jupiter Metis. This request concerns routing; token verification is separate.

Features and AMM design

Our curve derives from Meteora's Dynamic Bonding Curve code, with Evolut fee, authority and graduation changes, under mainnet program EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ.

New production launches select 1% or 2% curve trading fees, split 50/50 between creator and Evolut with integer rounding, and graduate at 85 SOL. Historical pools retain their original settings. Graduation creates an official Raydium CPMM pool and permanently locks the LP issued during graduation through Burn & Earn, preserving fee rights. Graduated markets use Raydium's routing integration; our adapter excludes completed curves.

Evolving coins retain their mint while names/artwork/story can develop at milestones. In the reviewed operational-PDA metadata path, the worker's rename instruction preserves the symbol. Classic creator-controlled metadata and treasury-controlled legacy metadata have broader authority, so we do not claim every platform token has a protocol-enforced immutable ticker. Program and external-program upgrade authorities remain trusted.

Adapter and testing

We implemented Jupiter's Amm interface for full-consumption ExactIn buys/sells between legacy SPL tokens and WSOL, using contract integer math and cached account updates with no networking in the adapter. The patched adapter's 27 offline tests passed separately against the captured historical deployed binary and the new local candidate, whose executable was subsequently matched on mainnet after upgrade. Jupiter's official kit compares six buys against an untouched 2%/85 SOL account snapshot, including the exact remaining threshold. Supplemental tests cover sells, fees, slippage rollback, account validation, repeated round trips and boundary amounts. The 1% and two-segment test states are synthetic. ExactOut, partial fills, Token-2022, referrals and variable fees are unsupported. Jupiter's final routing variant and CPI/multi-hop validation remain to be completed with your team.

Internal security review

We completed an internal, AI-assisted source review and adversarial testing of authorities, launches, swaps, claims, graduation and the adapter, with 121 passing cases across five suites against the previous baseline. This is not an independent external audit or certification. The report and remediation addendum record the scope, findings and limitations, including trusted metadata/upgrade authorities, external graduation dependencies and remaining dependency advisories. Backend payment binding and cloud infrastructure are outside the original report's scope.

Your guide asks for "Your security audit" without specifying an external auditor. Would this transparently labeled internal report be acceptable for integration review? If not, please tell us the additional scope/evidence or external review you require.

The addendum corrects the original Classic-fee interpretation: Classic launches were free at baseline, so there was no existing Classic charge to bypass. The owner subsequently approved a 0.015 SOL fee for both Classic and Evolving. The upgraded program now transfers that fee to the canonical treasury within new-pool initialization. Compatible dependencies were also updated, including matched patched integer arithmetic in execution and quoting. The remediation has separate host, actual-SBF, application and adapter test evidence. The owner-signed upgrade was verified against the tested executable, and the compatible backend is now live with release/quote and admission checks passed. Those checks sent no financial transaction and do not prove a paid launch or nonzero fee payout. Evolving's upfront AI/creation funding remains a separate transaction and is not an automatic-refund escrow.

The public [SDK repository](https://github.com/KidKnot/evolut-jupiter-amm) includes the adapter, source hashes, public account fixtures and reproducible tests. We disclose unresolved inherited-source terms in [source rights](https://github.com/KidKnot/evolut-jupiter-amm/blob/main/docs/SOURCE-RIGHTS.md); we do not claim a granted permissive license or present an upstream audit as an audit of our modifications. Please let us know what source-license terms your team requires to operate and maintain this integration.

Representative mainnet examples and traction

- Active curve: https://solscan.io/account/H8h1jyNiqG7vj7srt8RxAb7mtaaXa8WEb8VJPm1r3sT7
- Launched token, Mighty Great Dogwifhat (MGDW): https://solscan.io/token/Bro3jkhij9KzV4ExSaBAmiWxDsqBbsSAxTMEXWosEvoL
- Graduated example, The First Buyer (TDB): https://solscan.io/account/6TcPBmpVTJVaqPh2RsPKgXnmwbMToanH46YxqjvCfFCc

At finalized slot 462874125 on 29 October 2026, there were three existing program markets: one active production curve and two graduated historical 0.5 SOL test markets. Both graduated pools held 100% of their currently issued LP in the official locker. These are early testing examples, not organic-traction or live 85 SOL graduation claims.

Evolut is self-funded. Before Evolut, I worked on Solana NFT/memecoin botting and wallet recovery. My prior work includes ACO services for 2,000+ people, helping recover over $200K from compromised wallets, and over $1M in sniping results for others and private groups. These are founder-reported background figures, not Evolut trading volume, audited returns or an endorsement by another project.

Links and contact

- Current temporary mainnet site: https://website-production-2062.up.railway.app/
- Product notebook: https://www.evolut.fun/x/notebook
- SDK and reproducible tests: https://github.com/KidKnot/evolut-jupiter-amm
- Security review: https://github.com/KidKnot/evolut-jupiter-amm/blob/main/docs/INTERNAL-SECURITY-REVIEW-20261029.md
- Remediation addendum: https://github.com/KidKnot/evolut-jupiter-amm/blob/main/docs/REMEDIATION-ADDENDUM.md
- Current executable and rollout status: https://github.com/KidKnot/evolut-jupiter-amm/blob/main/docs/RELEASE-STATUS.md
- X: https://x.com/evolutdotfun
- Email: info@evolut.fun
- Program: https://solscan.io/account/EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ

Please advise whether technical review can begin with this evidence and what remains for routing integration and audit acceptance.

Thank you,
Kid Knot — Founder, Evolut

## Evidence retained with the application

See [internal review](INTERNAL-SECURITY-REVIEW-20261029.md),
[audit acceptance status](AUDIT-STATUS.md), [live pool evidence](LIVE-POOLS.md),
and [source rights](SOURCE-RIGHTS.md). The executable SHA-256 is
`885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6` for the captured deployed baseline. The separate local candidate is
`e9e70baeb58c545dae66b86f67c5ff44a79a77dc06a44a9a69654ae3a3dcf441`.
The latter hash and 1,142,312-byte length were matched to the upgraded program
in [separate finalized live verification](../evidence/mainnet-upgrade-verification-20261029.json).
The historical fixture/test records are unchanged; this program check does not
establish a new live pool capture. The compatible backend was subsequently
verified separately; see [application rollout verification](../evidence/application-rollout-verification-20261029.json).
The standalone package reproduces the 27 adapter tests; broader contract VM
tests use the separately retained operations harness. Exact suite evidence is
in `evidence/internal-review/AUDIT-EVIDENCE.json`.

The [official guide](https://developers.jup.ag/docs/swap/routing/amm/integration)
asks for a security audit without explicitly requiring an external auditor or
publishing internal-review acceptance criteria. Only Jupiter can confirm whether
this report meets its requirements. No form or message was submitted.
