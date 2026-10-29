# Evolut live LP lock proof

Checked: **29 October 2026, 5:33:45 PM IST** (2026-10-29T12:03:45.521Z); finalized mainnet snapshot slot **462874125**.

| Token | Status | LP supply inside official locker | LP outside locker |
| --- | --- | --- | --- |
| Emberback Cigar (PUFF) | Permanently locked | 100% | 0 |
| The First Buyer (TDB) | Permanently locked | 100% | 0 |
| Mighty Great Dogwifhat (MGDW) | Still on bonding curve | No Raydium LP yet | Not applicable |

The percentage compares the single shared locker LP vault with the current SPL LP mint supply. It does not count the same vault twice or confuse a Fee Key NFT with LP ownership. Raydium keeps an additional 100-unit minimum in its internal pool accounting which was never minted to a wallet.

## Emberback Cigar (PUFF)

- Mint: [8wKyaHRNrpxPYFGky9QKpNJ5fnnJg3HjLqtcxjGcEvoL](https://solscan.io/token/8wKyaHRNrpxPYFGky9QKpNJ5fnnJg3HjLqtcxjGcEvoL)
- Evolut source pool: [3wWJhR21G5cHBRJSNHBtcKBrxKmyX21vtdJPGCz1ah8j](https://solscan.io/account/3wWJhR21G5cHBRJSNHBtcKBrxKmyX21vtdJPGCz1ah8j)
- Raydium pool: [EzMHCLC1Y4d8LBJ4JZLphPTCWrqqwKfnsmqYNzy9RcXc](https://solscan.io/account/EzMHCLC1Y4d8LBJ4JZLphPTCWrqqwKfnsmqYNzy9RcXc)
- LP mint: [5Prdm1eAxHX1LynKPu3Eo5xoDSqHYWYETUGSM2yrwZkC](https://solscan.io/account/5Prdm1eAxHX1LynKPu3Eo5xoDSqHYWYETUGSM2yrwZkC)
- Locker LP vault: [FZPGDYjcjFU49cTZbm8U2dGxccv2HsVrJXQq8brbcSwq](https://solscan.io/account/FZPGDYjcjFU49cTZbm8U2dGxccv2HsVrJXQq8brbcSwq)
- LP supply: **190564050934 raw units**; locker balance: **190564050934 raw units**; outside locker: **0**.
- Evolut source LP account balance: **0**. No delegate or close authority is set on the locker vault; LP mint has no freeze authority.
- Verified successful graduation transaction: [2KKygK2d3d2yL7XxwZbuzAzdBkK5FYz4h4dveSG7ira9qAc8bWyj4Uy5EZypKQC1wrxXa9CUyEHsFyB9ro7jjm55](https://solscan.io/tx/2KKygK2d3d2yL7XxwZbuzAzdBkK5FYz4h4dveSG7ira9qAc8bWyj4Uy5EZypKQC1wrxXa9CUyEHsFyB9ro7jjm55) at slot 462145224; transaction includes both official lock CPIs.

| Fee side | Official lock account | Initially issued LP, raw | Current lock record, raw | Claimed fee LP, raw |
| --- | --- | --- | --- | --- |
| creator | [62szXNdCz2bMfBdXuM2CWVFrSJzxUwN8RSZtt338MQip](https://solscan.io/account/62szXNdCz2bMfBdXuM2CWVFrSJzxUwN8RSZtt338MQip) | 97629023450 | 92935027483 | 4693995967 |
| Evolut | [GAS7K4BDHV75n9ykDwyK1r6Jp5mRVEMzSgo2VcmASWDi](https://solscan.io/account/GAS7K4BDHV75n9ykDwyK1r6Jp5mRVEMzSgo2VcmASWDi) | 97629023451 | 97629023451 | 0 |

## The First Buyer (TDB)

- Mint: [Ek1ug36x75SqcWCdyD67tUdJuJYgfsSZoenpAeREvoL](https://solscan.io/token/Ek1ug36x75SqcWCdyD67tUdJuJYgfsSZoenpAeREvoL)
- Evolut source pool: [8uVAf8b8BhYZbQdzCHt8CfCW33Y9N25sHYkLz2yAwZX5](https://solscan.io/account/8uVAf8b8BhYZbQdzCHt8CfCW33Y9N25sHYkLz2yAwZX5)
- Raydium pool: [6TcPBmpVTJVaqPh2RsPKgXnmwbMToanH46YxqjvCfFCc](https://solscan.io/account/6TcPBmpVTJVaqPh2RsPKgXnmwbMToanH46YxqjvCfFCc)
- LP mint: [9a38tLRTHfZLyJLJopTFpLAKLLozZmu44jHnS3NJ3iGn](https://solscan.io/account/9a38tLRTHfZLyJLJopTFpLAKLLozZmu44jHnS3NJ3iGn)
- Locker LP vault: [GvDHdmbSWQC4W19DDC9prkuQTGKwHVh8c2HM25bfbPKB](https://solscan.io/account/GvDHdmbSWQC4W19DDC9prkuQTGKwHVh8c2HM25bfbPKB)
- LP supply: **195207751431 raw units**; locker balance: **195207751431 raw units**; outside locker: **0**.
- Evolut source LP account balance: **0**. No delegate or close authority is set on the locker vault; LP mint has no freeze authority.
- Verified successful graduation transaction: [3wd8Hz4M9hpFxQdQwRJXLhaNnXD8UNvpzxsUQHZCsu4VibqMYNwexHiMAz6NP3tVYadQz9D7EaYZ7L7a8FLT7T56](https://solscan.io/tx/3wd8Hz4M9hpFxQdQwRJXLhaNnXD8UNvpzxsUQHZCsu4VibqMYNwexHiMAz6NP3tVYadQz9D7EaYZ7L7a8FLT7T56) at slot 461340078; transaction includes both official lock CPIs.

| Fee side | Official lock account | Initially issued LP, raw | Current lock record, raw | Claimed fee LP, raw |
| --- | --- | --- | --- | --- |
| creator | [JV3dqMe29ecAhbLmtCWRe1KMTuTh4zb5aDdy8aeKpnw](https://solscan.io/account/JV3dqMe29ecAhbLmtCWRe1KMTuTh4zb5aDdy8aeKpnw) | 97629023450 | 97578727980 | 50295470 |
| Evolut | [H6HvPBxgyLNjaFB3jQwk3S9MKZ2Jf5PkLMBvdcQgEXSJ](https://solscan.io/account/H6HvPBxgyLNjaFB3jQwk3S9MKZ2Jf5PkLMBvdcQgEXSJ) | 97629023451 | 97629023451 | 0 |

## Mighty Great Dogwifhat (MGDW)

- Mint: [Bro3jkhij9KzV4ExSaBAmiWxDsqBbsSAxTMEXWosEvoL](https://solscan.io/token/Bro3jkhij9KzV4ExSaBAmiWxDsqBbsSAxTMEXWosEvoL)
- Evolut source pool: [H8h1jyNiqG7vj7srt8RxAb7mtaaXa8WEb8VJPm1r3sT7](https://solscan.io/account/H8h1jyNiqG7vj7srt8RxAb7mtaaXa8WEb8VJPm1r3sT7)
- Still on the bonding curve. No Raydium LP exists for this coin yet.

## What the proof establishes

Both lock PDAs are owned by the official [Raydium locker](https://solscan.io/account/LockrWmn6K5twhz3y9w1dQERbmgSaRkfnTeTKbpofwE) and bind to the correct pool, LP mint and Fee Key NFTs. All receipt-derived addresses and live account layouts were checked. Current fee claims can burn earned LP units, so current supply can differ from initial graduation amounts.

The saved graduation transactions were also checked offline: all four calls use the exact official lock instruction and the amounts recorded in the graduation receipts. The post-transaction locker balances equal all initially issued LP, and each Fee Key NFT was delivered to its receipt-designated creator or treasury. The lock record's original owner is Evolut's signing pool-authority PDA; that field is not the current NFT beneficiary. `report.json` distinguishes those identities.

Raydium documents Burn & Earn as permanent escrow with fee collection and no principal-withdrawal operation: https://docs.raydium.io/user-flows/burn-and-earn . This proves LP custody at the recorded slot, not absence of every token, upgrade or protocol risk.

Read-only RPC calls: 7. Transactions submitted: **0**.
Raw public account/transaction evidence SHA-256: 03e6c2bad60cf467431e363c89bda75542a196e48721d3f2aa327db3bf3bb78e. See [raw chain evidence](../evidence/lp-lock-chain-evidence.json) and [decoded report](../evidence/lp-lock-proof.json).

