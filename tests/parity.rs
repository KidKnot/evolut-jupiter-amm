//! Official Jupiter test-kit parity: local candidate SBF + finalized account snapshot.
//! No network feature is enabled in the test kit; these tests never submit a trade.
use evolut_jupiter_amm::{EvolutAmm, WSOL_MINT, encode_native_swap};
use jupiter_amm_interface::Swap;
use jupiter_amm_test_kit::{PoolTest, assert_pool_parity};
use sha2::{Digest, Sha256};
use solana_pubkey::{Pubkey, pubkey};

const POOL: Pubkey = pubkey!("H8h1jyNiqG7vj7srt8RxAb7mtaaXa8WEb8VJPm1r3sT7");
const BASE: Pubkey = pubkey!("Bro3jkhij9KzV4ExSaBAmiWxDsqBbsSAxTMEXWosEvoL");

fn encode(swap: &Swap, amount: u64) -> Vec<u8> {
    assert!(matches!(swap, Swap::Placeholder { data: None }));
    encode_native_swap(amount, 0)
}

#[test]
fn pinned_local_candidate_program_bytecode() {
    let binary = std::fs::read("tests/fixtures/evolut.so").unwrap();
    assert_eq!(binary.len(), 1_142_312);
    assert_eq!(
        format!("{:x}", Sha256::digest(&binary)),
        "e9e70baeb58c545dae66b86f67c5ff44a79a77dc06a44a9a69654ae3a3dcf441"
    );
}

#[test]
fn official_kit_mainnet_85sol_2pct_exact_input_buys() {
    let test = PoolTest::new(POOL)
        .add_swap(WSOL_MINT, BASE, 10_000)
        .add_swap(WSOL_MINT, BASE, 10_000_001)
        .add_swap(WSOL_MINT, BASE, 100_000_000)
        .add_swap(WSOL_MINT, BASE, 1_000_000_000)
        .add_swap(WSOL_MINT, BASE, 84_000_000_000)
        .add_swap(WSOL_MINT, BASE, 86_734_693_877); // exact remaining 85 SOL net threshold
    assert_pool_parity::<EvolutAmm>(&test, encode);
}
