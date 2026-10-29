# Source provenance

This Evolut curve implementation is derived from the supplied `moltrade-dbc`
workspace, which follows Meteora's Dynamic Bonding Curve code lineage. Existing
curve/math attribution comments have been retained. The fresh deployment replaces
the migration integration; it does not claim original authorship of retained math.

The supplied workspace contained no LICENSE, COPYING, or NOTICE file. No license
grant is asserted by this notice. Preserve any additional upstream license notices
provided with the original source before redistribution.

The Raydium adapter uses the external official CPMM and Burn & Earn deployments.
Its wire-layout references are:

- https://github.com/raydium-io/raydium-cp-swap
- https://github.com/raydium-io/raydium-cpi
- https://github.com/raydium-io/raydium-sdk-V2

Captured program/source hashes and retrieval provenance are recorded in
`server/vm-tests/fixtures/raydium/manifest.json` in the application repository.
Captured binaries, wallet keys, build outputs, and the temporary feasibility
program are not part of this production source distribution.
