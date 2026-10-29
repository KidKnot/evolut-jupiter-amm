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
const BUY_AMOUNT: u64 = 1_000_000_049;

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

    fn assert_rejected_atomically(&mut self, instruction: Instruction, error_name: &str) {
        let mut addresses = self.amm.get_accounts_to_update();
        addresses.extend([self.base_ata, self.quote_ata]);
        let before: Vec<_> = addresses
            .iter()
            .map(|address| self.svm.get_account(address).unwrap())
            .collect();
        let error = self.send(instruction).expect_err("native swap must reject");
        assert!(
            error.meta.logs.iter().any(|line| line.contains(error_name)),
            "expected {error_name}, got {:?}\n{}",
            error.err,
            error.meta.logs.join("\n")
        );
        for (address, account) in addresses.into_iter().zip(before) {
            assert_eq!(
                self.svm.get_account(&address).unwrap(),
                account,
                "failed swap changed account {address}; transaction SOL fee is excluded"
            );
        }
        self.refresh();
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

fn assert_round_trip(synthetic_one_percent: bool) {
    let mut harness = Harness::new(BUY_AMOUNT, synthetic_one_percent);
    assert_eq!(harness.balance(harness.base_ata), 0);
    let buy = harness.checked_swap(false, BUY_AMOUNT);
    assert_eq!(
        harness.balance(harness.quote_ata),
        0,
        "input funded with exactly quoted amount"
    );
    assert_eq!(
        buy.fee_amount % 2,
        1,
        "exercise the odd-lamport 50/50 remainder"
    );
    let sell_amount = buy.out_amount / 2;
    harness.checked_swap(true, sell_amount);
    assert_eq!(
        harness.balance(harness.base_ata),
        buy.out_amount - sell_amount
    );
}

#[test]
fn captured_mainnet_two_percent_buy_then_sell_matches_quote_and_fee_ledgers() {
    assert_round_trip(false);
}

#[test]
fn synthetic_fee_only_one_percent_variant_buy_then_sell_matches_quote_and_fee_ledgers() {
    assert_round_trip(true);
}

#[test]
fn buy_and_sell_slippage_rejections_preserve_all_pool_and_token_accounts() {
    let mut harness = Harness::new(BUY_AMOUNT, false);
    let buy = harness
        .amm
        .quote(&Harness::quote_params(false, BUY_AMOUNT))
        .unwrap();
    let instruction = harness.instruction(false, BUY_AMOUNT, buy.out_amount + 1);
    harness.assert_rejected_atomically(instruction, "ExceededSlippage");
    let buy = harness.checked_swap(false, BUY_AMOUNT);
    let amount = buy.out_amount / 2;
    let sell = harness
        .amm
        .quote(&Harness::quote_params(true, amount))
        .unwrap();
    let instruction = harness.instruction(true, amount, sell.out_amount + 1);
    harness.assert_rejected_atomically(instruction, "ExceededSlippage");
    harness.checked_swap(true, amount);
}

#[test]
fn above_85_sol_threshold_rejects_partial_fill_in_quote_and_native_execution() {
    let amount = 170_000_000_000;
    let mut harness = Harness::new(amount, false);
    let error = harness
        .amm
        .quote(&Harness::quote_params(false, amount))
        .unwrap_err();
    assert!(
        error.to_string().contains("SwapAmountIsOverAThreshold"),
        "{error}"
    );
    let instruction = harness.instruction(false, amount, 0);
    harness.assert_rejected_atomically(instruction, "SwapAmountIsOverAThreshold");
    assert_eq!(harness.balance(harness.quote_ata), amount);
    assert_eq!(harness.balance(harness.base_ata), 0);
    harness.checked_swap(false, BUY_AMOUNT);
}
