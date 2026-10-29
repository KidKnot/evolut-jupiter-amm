# Evolut Jupiter AMM SDK

Rust adapter for quoting Evolut’s Solana bonding curves through the Jupiter AMM interface.

**Mainnet program:** [`EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ`](https://solscan.io/account/EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ)

## Supported markets

- Legacy SPL tokens paired with wrapped SOL.
- Full-consumption ExactIn buys and sells, with contract integer math.
- Flat 1% and 2% curve fees, split 50/50 between creator and Evolut.
- Per-pool configuration; current production curves graduate at 85 SOL.
- Cached account updates with no network requests or signing inside the adapter.

Completed curves are excluded; graduated markets use Raydium CPMM. ExactOut,
partial fills, Token-2022 and variable-fee configurations are unsupported.

## Build and test

Tested with Rust 1.94.1. Install the platform’s C/C++ build tools, then run:

```sh
node scripts/verify-package.mjs
cargo test --locked --jobs 2
```

The 27 adapter tests use committed fixtures and run without RPC access. They cover
quote/execution parity, fee rounding, account validation and slippage rollback.
After dependencies are cached, add `--offline`.

On Windows with Visual Studio Build Tools:

```powershell
node run-cargo.mjs test --locked --offline --jobs 2
```

## Repository layout

| Path | Contents |
| --- | --- |
| `src/` | Jupiter interface implementation, account validation and quotes |
| `tests/` | Parity, execution and validation tests |
| `tests/fixtures/` | Account snapshots and program executable |
| `vendor/evolut-curve/` | Contract source used for layouts and curve math |
| `docs/` | Integration evidence, release details and security review |

The adapter currently uses `Swap::Placeholder`; Jupiter’s router mapping and
CPI/multi-hop integration are pending.

## Documentation

- [Release and deployed program](docs/RELEASE-STATUS.md)
- [Test coverage and fixtures](docs/ATOMIC-LAUNCH-CANDIDATE.md)
- [Internal security review](docs/INTERNAL-SECURITY-REVIEW-20261029.md) and [remediation addendum](docs/REMEDIATION-ADDENDUM.md)
- [Example pools and LP locks](docs/LIVE-POOLS.md)
- [Source provenance and terms](docs/SOURCE-RIGHTS.md)

## Contact

[info@evolut.fun](mailto:info@evolut.fun) · [@evolutdotfun](https://x.com/evolutdotfun)
