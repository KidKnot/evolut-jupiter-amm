# Internal security review: authorities, launches, metadata and curve claims

Date: 2026-10-29. This is an AI-assisted internal source review and adversarial test supplement. It is not an independent audit, certification, guarantee of safety or a claim that Jupiter accepts internal reviews.

## Scope and artifact

Reviewed source: `<review-workspace>/evolut-jupiter-amm/vendor/evolut-curve/programs/dynamic-bonding-curve/src`, with `mainnet` policy branches considered explicitly. Mainnet program: `EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ`.

The parent review owns source/binary identity and global test execution. The supplemental tests load the pinned binary through `output/matched-fee-contract-20261028/contract-artifacts.mjs` and pinned external fixture hashes through the existing harness. All tests are offline. No vendor/runtime edits, live transactions, private keys or live-service changes were made for this review.

Reviewed paths include `lib.rs` dispatch and `access_control.rs`; admin and fee-operator creation/revocation; creation of immutable configurations; SPL pool creation, metadata CPI and initial-buy quotation; operational metadata changes; creator transfer; creator, partner and protocol curve claims; creator/partner/protocol surplus; pool-creation fee claims; partner and pool metadata creation; token-transfer helpers; applicable configuration and fee-accounting methods. Raydium graduation, locked LP and post-graduation creator-fee accounting are covered by the separate migration review, not claimed as independently re-reviewed here.

## Result

No unauthorized fund-withdrawal, operational-authority takeover or cross-pool vault substitution exploit was established in this scope. This conclusion is bounded by the paths and evidence below. Two confirmed behavior/disclosure issues must not be represented as stronger on-chain guarantees than they are.

### A-01: launch payment is not an on-chain prerequisite

Classification: informational trust boundary; a monetization/control gap if the product promises that **every** on-chain pool creation must pay the website launch fee. No theft of existing customer funds is demonstrated.

Evidence:

- `instructions/partner/ix_create_config.rs:181` explicitly requires `pool_creation_fee == 0` on mainnet. This is deliberate to avoid double charging the backend-managed payment.
- `instructions/initialize_pool/paid_launch.rs:1` states that payments/retries are backend-managed; the only instruction there is a read-only quote. There is no on-chain payment receipt, escrow or ticket consumed by initialization.
- `instructions/initialize_pool/launch_authorization.rs:22` and `:26` make Classic creator/immutable metadata launches permissionless and require the actual partner/operator signer only for partner-managed modes.
- `instructions/initialize_pool/ix_initialize_virtual_pool_with_spl_token.rs:141` applies the launch signer check. Its only pool-creation-fee transfer starts at `:226`, and new mainnet configurations set that fee to zero.

Preconditions and impact: given an existing Classic configuration, an ordinary user can create a fresh mint/pool directly by paying network/account rent and signing as creator/payer/mint. They need no treasury or worker signature and need not send the separate website service fee. Whether an out-of-band pool is listed or receives AI/backend services depends on backend policy, outside this contract review. The amount avoided is the applicable website launch service charge; this review does not infer a current Classic fee amount from historical chat. Curve trading fees and graduation policy remain in the configuration and are not bypassed by this route.

Evolving launch distinction: an ordinary attacker cannot obtain the operational signer by supplying a public key or marking a nonsigner account. Omitting the operational PDA falls back to requiring the actual treasury signature; it is not an authorization bypass. A compromised/incorrectly implemented authorized worker could co-sign an unpaid evolving launch because payment proof is not checked on chain. Checking the server's payment-to-launch binding remains necessary before claiming a complete paid-launch security review.

Recommendation: describe the platform charge as a backend service charge unless atomic payment is implemented. If payment for all direct on-chain launches is a requirement, implement a reviewed on-chain transfer or single-use payment authorization; do not claim that the present read-only quote provides that guarantee. This audit does not change deployed economics or contract behavior.

Reproduction: supplemental test `Classic initialization is permissionless and transfers no platform launch fee` creates a second Classic pool with real generated user signatures and asserts the treasury's native SOL balance does not change during initialization. Separate negative tests cover absent, wrong and nonsigner evolving workers.

### A-02: fixed-ticker guarantee depends on metadata authority mode

Classification: informational integration disclosure; broad claims that every platform token has an immutable ticker are unsupported by this source.

Evidence:

- `instructions/partner/ix_create_config.rs:169` permits mainnet authority values 0 and 2. `state/config.rs:352` defines 0 as creator metadata authority and 2 as partner metadata authority.
- `state/config.rs:367` returns the creator for Classic metadata and the supplied partner for partner-managed metadata.
- `instructions/initialize_pool/process_create_token_metadata.rs:53` selects the operational PDA when supplied, otherwise the treasury/partner. The metadata remains mutable for these modes.
- `instructions/operator/ix_update_operational_metadata.rs:10` binds the canonical operational PDA to its current signer; `:14` binds the Metaplex metadata PDA/owner; `:33` checks its mint and authority. The CPI at `:42` changes the name while preserving symbol, URI, seller fee, creators, collection and uses.

Consequence: operational-PDA evolving tokens have a name-only update path in the current program, so that worker cannot change the symbol or URI through this instruction. Classic tokens assign full Metaplex update authority to the creator, who can use Metaplex directly to change metadata. A genuinely treasury-signed legacy evolving launch that omits the operational account assigns authority to treasury and retains its broader Metaplex powers. The program upgrade authority is a separate trusted power and can change future code behavior.

Recommendation: Jupiter material should identify the authority mode of the submitted token(s), and qualify the fixed-symbol statement to operational-PDA evolving tokens under the reviewed program. Do not describe mutable creator/treasury authority tokens as having a protocol-enforced permanent ticker. No name-conflict safeguard is proposed or added here; that was explicitly outside the user's intended policy.

Reproduction: supplemental tests inspect metadata authority after operational-PDA creation and after the legitimate treasury fallback. The metadata update test checks that a successful authorized rename preserves the prior symbol and URI and fails after worker revocation.

## Authorization and account-binding review

| Surface | Enforcement inspected | Result / residual trust |
|---|---|---|
| Operational authority initialization/rotation | `admin/ix_operational_authority.rs:7`, `:10`, `:16`, `:19`: canonical program PDA and fixed cold-admin signer; rejects treasury/admin as operational signer; zero revokes | Ordinary callers cannot initialize, rotate or revoke. The cold admin remains trusted to appoint a worker. |
| New configuration | `partner/ix_create_config.rs:218`: treasury must sign; fixed native mint and treasury leftover receiver; config is initialized, not updated | An attacker cannot create a new program-owned policy with arbitrary fee ownership. Existing policies remain immutable through exposed instructions. |
| Mainnet supply / mint authority | Mainnet config requires six decimals, initial 1 billion token units at that precision, no vesting and authority modes 0/2; initialization revokes mint authority via `get_mint_authority` at `initialize_pool/ix_initialize_virtual_pool_with_spl_token.rs:209` | Creator/operational metadata authority is not token mint authority in new mainnet policies. |
| Pool creation | Creator/payer/mint signers; pool PDA binds config and mint pair; vault PDAs bind mint/pool; config binds quote mint; fixed pool authority and Metaplex program | No account substitution path established. Metaplex enforces canonical creation metadata; initialization also protects pool-authority lamports/owner/size around metadata CPI. |
| Evolving worker authorization | Canonical operational PDA, program ownership/deserialization, nonzero worker, actual signer match in `initialize_pool/launch_authorization.rs` | Wrong/nonsigner worker rejected. Legacy treasury signature remains authorized intentionally. Backend payment discipline is trusted. |
| Operational rename | Pool/config link, canonical metadata owner/PDA, matching mint/update authority, current worker signer, fixed executable Metaplex program; name byte limit 32 | No arbitrary CPI, metadata URI/symbol write, treasury withdrawal or minting authority delegated. Worker can rename any eligible token in its authority domain, which is intentional. |
| Creator fee claim | `lib.rs:138` access control, `creator/ix_claim_creator_trading_fee.rs` creator Signer and `has_one` creator/base-vault/quote-vault/base-mint; typed token accounts; checked SPL transfer | Only current creator can claim. Creator may intentionally choose another same-mint destination. A wrong mint fails, and Solana rollback restores counters. |
| Partner fee claim | `lib.rs:100` binds signer to config fee claimer; pool binds config/vaults/mint; `partner/ix_claim_partner_trading_fee.rs` fixes both recipient ATAs to treasury | Neither worker nor even an authorized treasury claim can redirect destination through this instruction. |
| Protocol fee claim/operator | Admin access control at `lib.rs:54` and `:61`; operator state initialized under program PDA; claim access control at `:68` checks operator field and signer | Operator creation/revocation is admin-controlled. Operator accounts are program-owned/discriminator-checked. Missing seed checks on every use are not an exploit because no unauthorized construction path was established. Protocol payments are fixed treasury ATAs. |
| Surplus | Creator/partner/protocol access controls in `lib.rs:147`, `:120`, `:73`; pool/config/quote-vault binding; curve-completion check and one-shot flags | No unauthorized withdrawal established. All use accounted quote reserve above the threshold, not the raw account balance; split and flags prevent repeating an authorized withdrawal. Arithmetic is separately reviewed. |
| Pool creation fees | Config/pool binding; role checks; one-shot state flags; treasury receiver fixed | New mainnet configs force this fee to zero. Legacy fee-handling paths do not expose arbitrary recipients. |
| Creator transfer | `creator/ix_transfer_pool_creator.rs:14`, `:15`, `:21`, `:35`: current creator signer and config binding; allowed phases checked | Transfers residual curve rights. Post-graduation fee-key ownership remains separate. Sending creator rights to a zero/unusable address can self-lock those rights; no third-party exploit demonstrated. |
| Partner/pool metadata | Creator-only pool metadata dispatch at `lib.rs:130`; metadata PDA includes pool; partner metadata PDA includes signing fee claimer | A user can publish their own partner metadata but cannot overwrite another signer/pool's canonical account. Strings are untrusted display data for consumers, not executable contract logic. |

## Test supplement and execution status

Test file: `<review-workspace>/output/internal-security-audit-20261029/authorities-vm/authorities-launch.test.ts`.

The helper is a test-only copy of the existing pinned LiteSVM fixture setup, with imports adjusted to the reviewed artifacts. It does not mock the Evolut program: it executes its SBF. Known treasury/admin signatures are substituted only for offline setup/authorized-role scenarios because their private keys are not used. Attacker, creator and operational worker transactions use generated keypairs and signature verification. This proves instruction-level checks under synthetic state, not live deployment configuration, RPC behavior, private-key custody, operational security or exhaustive financial safety.

Ten focused cases cover:

1. Unauthorized configuration creation with no created policy account.
2. Direct Classic pool initialization without platform payment.
3. Missing evolving authorizer and transaction rollback.
4. Nonsigner/wrong evolving worker rejected; correct worker accepted.
5. Revoked worker rejected; genuine treasury legacy path remains authorized.
6. Operational rename preserves symbol/URI, rejects unrelated user and revoked worker.
7. Unauthorized creator claim, wrong-mint payout rollback, exact legitimate payout and zero repeat payout.
8. Cross-pool quote-vault substitution rejected.
9. Treasury claim destination redirection rejected and legitimate fixed-recipient payout exact.
10. Creator-rights transfer rejects an outsider; old creator loses claim permission and new creator gains it.

Execution result: **10 passed, 0 failed, 0 skipped**, exit code 0, 2026-10-29, WSL Ubuntu / installed Node + LiteSVM. TAP duration was 10,817.805016 ms. The optional bigint native binding was unavailable and the library used its stated pure-JavaScript fallback; LiteSVM's actual SBF execution completed. The binary SHA-256 was `885d9c887801162ea921ad9426e109bd87348f40ced68ed74fe7b4b8960103b6`. Test source SHA-256 was `0d36143ceb7d5e5474fbadb67d1986a4407dcddb0de2e74e84e5eb836a65b1bc`; helper SHA-256 was `d7218459664f5b7a2be0f6d4ecbc010ab8e1a7ebfb30d5f161e4a8729904f3e3`.

PowerShell run command:

```powershell
wsl.exe -d Ubuntu -- bash -lc 'cd <review-workspace>/output/internal-security-audit-20261029/authorities-vm && EVOLUT_VM_NETWORK=mainnet node --import tsx --test authorities-launch.test.ts'
```

The audit-local `node_modules` junction points to the existing test dependencies; no packages were installed. Native Windows execution lacks the litesvm Windows binary; the existing Linux binary in WSL was used successfully. This tooling distinction is unrelated to contract success or failure.

## Follow-up tests with material value

- Backend payment/signature replay test: one receipt cannot authorize a different creator, mint, configuration, launch id or total; expired blockhash refresh retains the binding. This cannot be proven from the on-chain source alone.
- Broaden wrong-owner/wrong-discriminator operational metadata tests and replace the supplied metadata program with an unrelated executable; verify failures and unchanged state.
- Exercise all three surplus role checks and one-shot flags with naturally reached nonzero surplus; compare aggregate receipts to accounted surplus. The current supplement does not claim that coverage.
- Fee-operator lifecycle: unauthorized creation/close, authorized close, reuse of closed account and substituted operator state with a wrong program owner.
- Classic metadata mutability should be tested/documented explicitly wherever product or integration copy describes ticker permanence.

No deployment or contract modification is recommended solely to make this report look stronger. Treat observed intended trust boundaries separately from exploitable permission failures, and use an independent audit if Jupiter requires one.
