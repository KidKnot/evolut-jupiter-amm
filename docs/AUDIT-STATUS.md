# Audit status and question for Jupiter

An **internal, AI-assisted security review** was completed on 29 October 2026:
[report and findings](INTERNAL-SECURITY-REVIEW-20261029.md). It includes source
inspection, 121 passing cases across five suites and dependency advisory triage.
That historical report covers the previous `885d9c88…` executable. The
[remediation addendum](REMEDIATION-ADDENDUM.md) corrects the Classic-fee
interpretation and records subsequent fee/dependency changes and their tests.
The new `e9e70bae…` executable was separately matched on mainnet after upgrade;
see [release status](RELEASE-STATUS.md). The compatible backend is live, with
separate release/quote and admission checks. These sent no financial transaction
and do not establish a paid launch or nonzero fee payout.
The review has open findings and limitations. Evolut has **no independent external
contract audit**; this report must not be described as one.

The [Jupiter integration guide](https://developers.jup.ag/docs/swap/routing/amm/integration)
asks applicants to include “Your security audit.” It does **not explicitly
require an external/independent auditor**. That guide and the
[AMM integration form](https://support.jup.ag/requests/new/amm-integrators) do not
publish an approved auditor list, minimum price, required report template or an
explicit acceptance rule for internal reviews. We should ask Jupiter whether this
report meets their requirements, rather than assume acceptance or rejection.

The prepared application asks Jupiter whether:

- This clearly labeled internal report is acceptable for integration review.
- They require additional evidence, a particular scope, or an external review.
- Technical adapter review can proceed while open findings and inherited-source
  terms are resolved.

These are draft questions, not an accepted exception. No question or report has
been sent externally, and no external auditor has been hired.

## Scope material ready for an auditor

- Program ID, exact deployed SBF hash/length, release source manifest and IDL.
- Jupiter quote/account adapter and its native-execution parity tests.
- Evolut changes affecting 1%/2% fees, the 50/50 split, rounding and claims.
- Operational/admin/upgrade authorities and metadata update permissions.
- Raydium graduation CPI identities, creation permissions, LP-lock verification,
  creator/treasury fee rights, setup reimbursement, fixed revenue and surplus burn.
- Safe state transitions and replay handling at graduation, including historical
  test configurations versus current 85 SOL production settings.
- Disclosure that only native top-level adapter execution is proven here;
  Jupiter's final CPI/multi-hop integration remains separate.

An acceptable report should identify its auditor, exact scope and commit, findings
and their disposition, and correspondence to the deployed binary. Preserve those
facts rather than presenting a generic badge or an unmatched upstream report.
