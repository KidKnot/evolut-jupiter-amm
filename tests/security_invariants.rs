//! Execute only in local LiteSVM against the explicitly pinned local candidate.
//! The 2% test uses captured accounts unchanged; the explicitly named 1% test
//! changes just the fee numerator in local memory and is not a mainnet snapshot.

use dynamic_bonding_curve::state::{PoolConfig, VirtualPool};
use evolut_jupiter_amm::{
    ASSOCIATED_TOKEN_PROGRAM_ID, EvolutAmm, PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
    associated_token_address,
};
use jupiter_amm_interface::{
    Amm, AmmContext, KeyedAccount, Quote, QuoteParams, SwapMode, SwapParams,
};
use jupiter_amm_test_kit::PoolSnapshot;
use litesvm::{LiteSVM, types::TransactionResult};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::{Pubkey, pubkey};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{collections::HashMap, path::PathBuf};

const POOL: Pubkey = pubkey!("H8h1jyNiqG7vj7srt8RxAb7mtaaXa8WEb8VJPm1r3sT7");
const BASE: Pubkey = pubkey!("Bro3jkhij9KzV4ExSaBAmiWxDsqBbsSAxTMEXWosEvoL");
const ROUTER: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

struct Harness {
    svm: LiteSVM,
    amm: EvolutAmm,
    wallet: Keypair,
    base_ata: Pubkey,
    quote_ata: Pubkey,
    base_vault: Pubkey,
    quote_vault: Pubkey,
    numerator: u64,
}

fn key(key: anchor_lang::prelude::Pubkey) -> Pubkey {
    Pubkey::new_from_array(key.to_bytes())
}

impl Harness {
    fn new(initial_wsol: u64, synthetic_one_percent: bool) -> Self {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let directory = fixtures.join("accounts").join(POOL.to_string());
        let snapshot = PoolSnapshot::load_dir(&directory).unwrap();
        let context = AmmContext {
            clock_ref: snapshot.clock().expect("captured mainnet clock").into(),
        };
        let amm = EvolutAmm::from_keyed_account(
            &KeyedAccount {
                key: POOL,
                account: snapshot.get(&POOL).unwrap().clone(),
                params: None,
            },
            &context,
        )
        .unwrap();
        let pool: VirtualPool =
            bytemuck::pod_read_unaligned(&snapshot.get(&POOL).unwrap().data[8..]);
        let mut svm = LiteSVM::new();
        svm.add_program_from_file(PROGRAM_ID, fixtures.join("evolut.so"))
            .expect("reviewed local candidate Evolut binary");
        // PoolSnapshot intentionally exposes lookup, not iteration. Read its
        // documented bincode Account files to seed every captured dependency.
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|value| value.to_str()) != Some("bin") {
                continue;
            }
            let address: Pubkey = path.file_stem().unwrap().to_str().unwrap().parse().unwrap();
            let account: Account = bincode::deserialize(&std::fs::read(path).unwrap()).unwrap();
            if !account.executable {
                svm.set_account(address, account).unwrap();
            }
        }
        let mut config_account = svm.get_account(&key(pool.config)).unwrap();
        let mut config: PoolConfig = bytemuck::pod_read_unaligned(&config_account.data[8..]);
        assert_eq!(config.pool_fees.base_fee.cliff_fee_numerator, 20_000_000);
        assert_eq!(config.creator_trading_fee_percentage, 50);
        assert_eq!(config.migration_quote_threshold, 85_000_000_000);
        if synthetic_one_percent {
            // Synthetic fee-only variant: retain config address and pool PDA.
            config.pool_fees.base_fee.cliff_fee_numerator = 10_000_000;
            config_account.data[8..].copy_from_slice(bytemuck::bytes_of(&config));
            svm.set_account(key(pool.config), config_account).unwrap();
        }
        let wallet = Keypair::new();
        svm.airdrop(&wallet.pubkey(), 2_000_000_000).unwrap();
        let quote_ata = create_funded_ata(&mut svm, &wallet, WSOL_MINT, initial_wsol);
        let base_ata = create_funded_ata(&mut svm, &wallet, BASE, 0);
        let mut harness = Self {
            svm,
            amm,
            wallet,
            base_ata,
            quote_ata,
            base_vault: key(pool.base_vault),
            quote_vault: key(pool.quote_vault),
            numerator: config.pool_fees.base_fee.cliff_fee_numerator,
        };
        harness.refresh();
        harness
    }

    fn refresh(&mut self) {
        let accounts: HashMap<Pubkey, Box<Account>> = self
            .amm
            .get_accounts_to_update()
            .into_iter()
            .map(|address| (address, Box::new(self.svm.get_account(&address).unwrap())))
            .collect();
        self.amm
            .update(&accounts)
            .expect("refresh from executed state");
    }

    fn pool(&self) -> VirtualPool {
        bytemuck::pod_read_unaligned(&self.svm.get_account(&POOL).unwrap().data[8..])
    }

    fn balance(&self, address: Pubkey) -> u64 {
        u64::from_le_bytes(
            self.svm.get_account(&address).unwrap().data[64..72]
                .try_into()
                .unwrap(),
        )
    }

    fn quote_params(sell: bool, amount: u64) -> QuoteParams {
        QuoteParams {
            amount,
            input_mint: if sell { BASE } else { WSOL_MINT },
            output_mint: if sell { WSOL_MINT } else { BASE },
            swap_mode: SwapMode::ExactIn,
            flow_type: Default::default(),
        }
    }

    fn instruction(&self, sell: bool, amount: u64, minimum: u64) -> Instruction {
        let user = self.wallet.pubkey();
        self.amm
            .native_swap_instruction(&SwapParams {
                swap_mode: SwapMode::ExactIn,
                in_amount: amount,
                out_amount: minimum,
                source_mint: if sell { BASE } else { WSOL_MINT },
                destination_mint: if sell { WSOL_MINT } else { BASE },
                source_token_account: if sell { self.base_ata } else { self.quote_ata },
                destination_token_account: if sell { self.quote_ata } else { self.base_ata },
                token_transfer_authority: user,
                user,
                payer: user,
                quote_mint_to_referrer: None,
                jupiter_program_id: &ROUTER,
                missing_dynamic_accounts_as_default: false,
            })
            .unwrap()
    }

    fn send(&mut self, instruction: Instruction) -> TransactionResult {
        self.svm.expire_blockhash();
        let instructions = [
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            instruction,
        ];
        self.svm
            .send_transaction(Transaction::new_signed_with_payer(
                &instructions,
                Some(&self.wallet.pubkey()),
                &[&self.wallet],
                self.svm.latest_blockhash(),
            ))
    }

    fn checked_swap(&mut self, sell: bool, amount: u64) -> Quote {
        let quote = self.amm.quote(&Self::quote_params(sell, amount)).unwrap();
        let before = self.pool();
        let [
            base_before,
            quote_before,
            base_vault_before,
            quote_vault_before,
        ] = [
            self.base_ata,
            self.quote_ata,
            self.base_vault,
            self.quote_vault,
        ]
        .map(|address| self.balance(address));
        let instruction = self.instruction(sell, amount, quote.out_amount);
        self.send(instruction)
            .expect("native swap with exact quoted minimum succeeds");
        let after = self.pool();
        let [base_after, quote_after, base_vault_after, quote_vault_after] = [
            self.base_ata,
            self.quote_ata,
            self.base_vault,
            self.quote_vault,
        ]
        .map(|address| self.balance(address));
        assert_eq!(quote.in_amount, amount);
        assert_eq!(quote.fee_mint, WSOL_MINT);
        assert!(quote.out_amount > 0);

        if sell {
            assert_eq!(
                base_before - base_after,
                quote.in_amount,
                "exact input debit"
            );
            assert_eq!(
                quote_after - quote_before,
                quote.out_amount,
                "exact output credit"
            );
            assert_eq!(base_vault_after - base_vault_before, quote.in_amount);
            assert_eq!(quote_vault_before - quote_vault_after, quote.out_amount);
            assert_eq!(after.base_reserve - before.base_reserve, quote.in_amount);
            assert_eq!(
                before.quote_reserve - after.quote_reserve,
                quote.out_amount + quote.fee_amount
            );
            assert!(after.sqrt_price < before.sqrt_price);
        } else {
            assert_eq!(
                quote_before - quote_after,
                quote.in_amount,
                "exact input debit"
            );
            assert_eq!(
                base_after - base_before,
                quote.out_amount,
                "exact output credit"
            );
            assert_eq!(quote_vault_after - quote_vault_before, quote.in_amount);
            assert_eq!(base_vault_before - base_vault_after, quote.out_amount);
            assert_eq!(before.base_reserve - after.base_reserve, quote.out_amount);
            assert_eq!(
                after.quote_reserve - before.quote_reserve,
                quote.in_amount - quote.fee_amount
            );
            assert!(after.sqrt_price > before.sqrt_price);
        }

        // Independently check integer fee rounding and the deployed 0/50/50 split.
        let gross_quote = if sell {
            quote.out_amount + quote.fee_amount
        } else {
            quote.in_amount
        };
        let expected_fee = ((u128::from(gross_quote) * u128::from(self.numerator) + 999_999_999)
            / 1_000_000_000) as u64;
        assert_eq!(quote.fee_amount, expected_fee);
        assert_eq!(
            after.protocol_quote_fee, before.protocol_quote_fee,
            "no protocol skim"
        );
        assert_eq!(
            after.creator_quote_fee - before.creator_quote_fee,
            expected_fee / 2
        );
        assert_eq!(
            after.partner_quote_fee - before.partner_quote_fee,
            expected_fee - expected_fee / 2
        );
        assert_eq!(
            after.metrics.total_trading_quote_fee - before.metrics.total_trading_quote_fee,
            expected_fee
        );
        assert_eq!(
            after.metrics.total_protocol_quote_fee,
            before.metrics.total_protocol_quote_fee
        );
        assert_eq!(after.protocol_base_fee, before.protocol_base_fee);
        assert_eq!(after.partner_base_fee, before.partner_base_fee);
        assert_eq!(after.creator_base_fee, before.creator_base_fee);
        assert_eq!(
            base_vault_after - after.base_reserve,
            base_vault_before - before.base_reserve
        );
        assert_eq!(
            quote_vault_after - after.quote_reserve,
            quote_vault_before - before.quote_reserve + expected_fee,
            "fees remain in the WSOL vault, outside tradable reserves"
        );
        self.refresh();
        quote
    }
}

// Same ATA-program creation path as the official test kit. Only local starting
// WSOL is seeded; all sell-side base tokens must come from a successful buy.
fn create_funded_ata(svm: &mut LiteSVM, wallet: &Keypair, mint: Pubkey, amount: u64) -> Pubkey {
    let user = wallet.pubkey();
    let ata = associated_token_address(user, mint);
    let instruction = Instruction {
        program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(user, true),
            AccountMeta::new(ata, false),
            AccountMeta::new_readonly(user, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data: vec![0],
    };
    svm.send_transaction(Transaction::new_signed_with_payer(
        &[instruction],
        Some(&user),
        &[wallet],
        svm.latest_blockhash(),
    ))
    .expect("create ATA through its native program");
    if amount > 0 {
        assert_eq!(mint, WSOL_MINT, "do not seed base tokens");
        let mut account = svm.get_account(&ata).unwrap();
        account.data[64..72].copy_from_slice(&amount.to_le_bytes());
        account.lamports += amount;
        svm.set_account(ata, account).unwrap();
    }
    ata
}

// Adversarial additions for the INTERNAL 2026-10-29 audit. This reuses a copy
// of the existing offline execution harness without changing shipped code or
// existing tests. Synthetic changes are confined to LiteSVM memory.
#[test]
fn repeated_round_trips_cannot_extract_quote_and_preserve_exact_fee_ledgers() {
    for one_percent in [false, true] {
        let mut h = Harness::new(200_000_000_000, one_percent);
        let mut seed = 0x73e8c341975bd026u64;
        let fixed = [
            2,
            49,
            50,
            51,
            99,
            100,
            101,
            10_000,
            1_000_001,
            1_000_000_049,
            20_000_000_000,
            84_000_000_000,
        ];
        let mut successful_sells = 0;
        for index in 0..28 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let amount = if index < fixed.len() {
                fixed[index]
            } else {
                1_000_000 + seed % 2_000_000_000
            };
            let before_quote = h.balance(h.quote_ata);
            let before_base = h.balance(h.base_ata);
            let buy = h.checked_swap(false, amount);
            let sell_quote = h.amm.quote(&Harness::quote_params(true, buy.out_amount));
            if sell_quote.is_err() {
                // Sub-lamport proceeds are intentionally not routed by the adapter.
                assert!(amount <= 2, "unexpected rejected sell at input {amount}");
                continue;
            }
            let sell = h.checked_swap(true, buy.out_amount);
            successful_sells += 1;
            assert!(sell.out_amount <= amount, "profitable immediate round-trip");
            assert!(h.balance(h.quote_ata) <= before_quote, "net WSOL created");
            assert_eq!(
                h.balance(h.base_ata),
                before_base,
                "all purchased base returned"
            );
            let pool = h.pool();
            let backed = pool
                .quote_reserve
                .checked_add(pool.partner_quote_fee)
                .unwrap()
                .checked_add(pool.creator_quote_fee)
                .unwrap()
                .checked_add(pool.protocol_quote_fee)
                .unwrap();
            assert!(
                h.balance(h.quote_vault) >= backed,
                "reserve or fee claims unbacked"
            );
        }
        assert_eq!(
            successful_sells, 27,
            "only the two-lamport dust sell is unroutable"
        );
    }
}

#[test]
fn whole_u64_input_range_is_rejected_or_quoted_without_panic() {
    let mut h = Harness::new(10_000_000_000, false);
    h.checked_swap(false, 1_000_000_049);
    let mut amounts = vec![
        0,
        1,
        2,
        49,
        50,
        51,
        99,
        100,
        101,
        u32::MAX as u64,
        85_000_000_000,
        86_734_693_877,
        86_734_693_878,
        u64::MAX / 2,
        u64::MAX - 1,
        u64::MAX,
    ];
    let mut seed = 0xe4a1ce579bb7d7f3u64;
    for _ in 0..128 {
        seed = seed
            .wrapping_mul(2862933555777941757)
            .wrapping_add(3037000493);
        amounts.push(seed);
    }
    for amount in amounts {
        for sell in [false, true] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                h.amm.quote(&Harness::quote_params(sell, amount))
            }));
            assert!(result.is_ok(), "quote panic: amount={amount}, sell={sell}");
            if let Ok(quote) = result.unwrap() {
                assert_eq!(quote.in_amount, amount);
                assert!(quote.out_amount > 0);
                assert_eq!(quote.fee_mint, WSOL_MINT);
                let gross = if sell {
                    quote.out_amount.checked_add(quote.fee_amount).unwrap()
                } else {
                    quote.in_amount
                };
                let expected =
                    (u128::from(gross) * u128::from(h.numerator)).div_ceil(1_000_000_000) as u64;
                assert_eq!(quote.fee_amount, expected);
            }
        }
    }
}

#[test]
fn fee_ceiling_and_half_split_conserve_every_lamport_at_integer_extremes() {
    use dynamic_bonding_curve::state::PoolFeesConfig;
    let mut config = PoolConfig::default();
    config.creator_trading_fee_percentage = 50;
    for numerator in [10_000_000u64, 20_000_000] {
        for amount in [
            0,
            1,
            2,
            49,
            50,
            51,
            99,
            100,
            101,
            1_000_000_049,
            u32::MAX as u64,
            u64::MAX / 2,
            u64::MAX - 1,
            u64::MAX,
        ] {
            let (net, fee) = PoolFeesConfig::get_excluded_fee_amount(numerator, amount).unwrap();
            let independent =
                (u128::from(amount) * u128::from(numerator)).div_ceil(1_000_000_000) as u64;
            assert_eq!(fee, independent);
            assert_eq!(net.checked_add(fee), Some(amount));
            let split = config.split_partner_and_creator_fee(fee).unwrap();
            assert_eq!(split.creator_fee, fee / 2);
            assert_eq!(split.partner_fee, fee - fee / 2);
            assert_eq!(split.partner_fee.checked_add(split.creator_fee), Some(fee));
            assert!(split.partner_fee - split.creator_fee <= 1);
        }
    }
}

#[test]
fn synthetic_two_segment_curve_crosses_both_directions_with_native_parity() {
    let mut h = Harness::new(100_000_000_000, false);
    let config_key = key(h.pool().config);
    let mut account = h.svm.get_account(&config_key).unwrap();
    let mut config: PoolConfig = bytemuck::pod_read_unaligned(&account.data[8..]);
    let original = config.curve[0];
    assert!(original.sqrt_price > config.sqrt_start_price);
    assert_eq!(config.curve[1].liquidity, 0, "fixture uses one segment");
    // An identical-liquidity subdivision exercises segment boundaries without
    // changing the continuous curve economics (integer rounding may differ).
    let midpoint = config.sqrt_start_price + (original.sqrt_price - config.sqrt_start_price) / 2;
    config.curve[0].sqrt_price = midpoint;
    config.curve[1] = original;
    account.data[8..].copy_from_slice(bytemuck::bytes_of(&config));
    h.svm.set_account(config_key, account).unwrap();
    h.refresh();
    let buy = h.checked_swap(false, 70_000_000_000);
    assert!(
        h.pool().sqrt_price > midpoint,
        "buy crossed segment boundary"
    );
    let sell = h.checked_swap(true, buy.out_amount);
    assert!(
        h.pool().sqrt_price < midpoint,
        "sell crossed segment boundary"
    );
    assert!(sell.out_amount < buy.in_amount, "round-trip must pay fees");
}
