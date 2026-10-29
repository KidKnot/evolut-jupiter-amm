# Evolut curve v1

Fresh Raydium-only curve deployment. Devnet program:
`FHrKE2Pibu6Bd2Y9pHyjkHkEWeftTcy1DhN63dDwQa97`.

The original curve math and creator-token vesting remain. Exact devnet trading
fee is 1%; the separately compiled `mainnet` feature pins official 1%/2% fee
configurations. Config creation requires the canonical Evolut treasury signer;
classic launch and swaps are permissionless, while evolving launch additionally
requires that treasury's authorization.

`graduate(nonce[32])` initializes official CPMM and two permanent Burn & Earn
locks atomically. All issued LP is split floor/remainder 50/50. Actual measured
setup rent/fee is capped by immutable config and reimbursed as existing-ATA WSOL.
No metadata is created for fee NFTs. Fee NFT ownership, not the initial receipt
beneficiary field, controls subsequent fee claims.

The generated ABI is `idl/evolut_curve.json`. Payload sizes are PoolConfig1040
and VirtualPool752 bytes, excluding the8-byte discriminator. Account layouts and
instruction/event discriminators should be read from the IDL.

Build locally with `cargo-build-sbf --manifest-path programs/dynamic-bonding-curve/Cargo.toml`.
The application repository's `server/vm-tests/build-production-contract.mjs`
provides the installed Windows toolchain environment and compiler-derived IDL
generation. See `server/vm-tests/PRODUCTION_CURVE_PROOF.md` for verified limits.

No deployment keys are included. This program was finalized on devnet at slot
497362284; the public release addresses and binary hash are recorded in
`server/scripts/fixtures/devnet-raydium-release.json` in the application repository.
Future upgrades must select the reviewed program key explicitly. Never use an old
default `target/deploy/*-keypair.json` from another checkout. Mainnet is not deployed.

See `UPSTREAM-NOTICES.md`; retained upstream attribution is not a new license grant.
