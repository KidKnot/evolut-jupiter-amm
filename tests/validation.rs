//! Offline boundary tests against the captured public mainnet account layout.
//! Mutations below are local test cases, never accounts submitted to a network.

use dynamic_bonding_curve::state::{PoolConfig, VirtualPool};
use evolut_jupiter_amm::{
    CONFIG_ACCOUNT_LEN, EvolutAmm, POOL_ACCOUNT_LEN, PROGRAM_ID, SWAP_DISCRIMINATOR,
    TOKEN_PROGRAM_ID, WSOL_MINT, encode_native_swap, event_authority, pool_authority,
};
use jupiter_amm_interface::{
    Amm, AmmContext, AmmError, KeyedAccount, QuoteMintToReferrer, QuoteParams, Swap, SwapMode,
    SwapParams,
};
use jupiter_amm_test_kit::PoolSnapshot;
use rust_decimal::Decimal;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_pubkey::{Pubkey, pubkey};
use std::{collections::HashMap, path::PathBuf, sync::atomic::Ordering};

const POOL: Pubkey = pubkey!("H8h1jyNiqG7vj7srt8RxAb7mtaaXa8WEb8VJPm1r3sT7");
const BASE: Pubkey = pubkey!("Bro3jkhij9KzV4ExSaBAmiWxDsqBbsSAxTMEXWosEvoL");
const ROUTER: Pubkey = Pubkey::new_from_array([91; 32]);
type Accounts = HashMap<Pubkey, Box<Account>>;

struct Fixture {
    amm: EvolutAmm,
    accounts: Accounts,
    context: AmmContext,
}

impl Fixture {
    fn unloaded() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/accounts")
            .join(POOL.to_string());
        let snapshot = PoolSnapshot::load_dir(path).expect("imported captured mainnet fixture");
        let context = AmmContext {
            clock_ref: snapshot.clock().expect("captured clock").into(),
        };
        let amm = EvolutAmm::from_keyed_account(
            &KeyedAccount {
                key: POOL,
                account: snapshot.get(&POOL).expect("captured pool").clone(),
                params: None,
            },
            &context,
        )
        .expect("captured pool has the supported deployed ABI");
        let accounts = amm
            .get_accounts_to_update()
            .into_iter()
            .map(|key| {
                (
                    key,
                    Box::new(snapshot.get(&key).expect("dependency").clone()),
                )
            })
            .collect();
        Self {
            amm,
            accounts,
            context,
        }
    }

    fn loaded() -> Self {
        let mut fixture = Self::unloaded();
        fixture
            .amm
            .update(&fixture.accounts)
            .expect("captured dependencies");
        assert!(fixture.amm.is_active(), "the reference pool must be active");
        assert!(fixture.amm.quote(&buy(1_000_000)).unwrap().out_amount > 0);
        fixture
    }

    fn pool(&self) -> VirtualPool {
        bytemuck::pod_read_unaligned(&self.accounts[&POOL].data[8..])
    }

    fn config_key(&self) -> Pubkey {
        contract_key(self.pool().config)
    }

    fn config(&self) -> PoolConfig {
        bytemuck::pod_read_unaligned(&self.accounts[&self.config_key()].data[8..])
    }

    fn change_pool(&mut self, change: impl FnOnce(&mut VirtualPool)) {
        let mut pool = self.pool();
        change(&mut pool);
        self.accounts.get_mut(&POOL).unwrap().data[8..].copy_from_slice(bytemuck::bytes_of(&pool));
    }

    fn change_config(&mut self, change: impl FnOnce(&mut PoolConfig)) {
        let key = self.config_key();
        let mut config = self.config();
        change(&mut config);
        self.accounts.get_mut(&key).unwrap().data[8..].copy_from_slice(bytemuck::bytes_of(&config));
    }

    fn assert_unavailable(&self) {
        assert!(!self.amm.is_active());
        assert!(self.amm.quote(&buy(1_000_000)).is_err());
        assert!(self.amm.native_swap_instruction(&swap()).is_err());
    }
}

fn contract_key(key: anchor_lang::prelude::Pubkey) -> Pubkey {
    Pubkey::new_from_array(key.to_bytes())
}

fn buy(amount: u64) -> QuoteParams {
    QuoteParams {
        amount,
        input_mint: WSOL_MINT,
        output_mint: BASE,
        swap_mode: SwapMode::ExactIn,
        flow_type: Default::default(),
    }
}

fn swap<'a>() -> SwapParams<'a, 'static> {
    SwapParams {
        swap_mode: SwapMode::ExactIn,
        in_amount: 1_000_000,
        out_amount: 1,
        source_mint: WSOL_MINT,
        destination_mint: BASE,
        source_token_account: Pubkey::new_from_array([11; 32]),
        destination_token_account: Pubkey::new_from_array([12; 32]),
        token_transfer_authority: Pubkey::new_from_array([13; 32]),
        user: Pubkey::new_from_array([14; 32]),
        payer: Pubkey::new_from_array([15; 32]),
        quote_mint_to_referrer: None,
        jupiter_program_id: &ROUTER,
        missing_dynamic_accounts_as_default: false,
    }
}

fn assert_error<T>(result: Result<T, AmmError>, fragment: &str) {
    match result {
        Ok(_) => panic!("expected rejection containing {fragment:?}"),
        Err(error) => assert!(
            error.to_string().contains(fragment),
            "expected {fragment:?}, got {error}"
        ),
    }
}

#[test]
fn constructor_rejects_wrong_owner_discriminator_size_and_executable_state() {
    let fixture = Fixture::unloaded();
    assert_eq!(fixture.accounts[&POOL].data.len(), POOL_ACCOUNT_LEN);
    let cases: &[(&str, fn(&mut Account), &str)] = &[
        ("owner", |a| a.owner = TOKEN_PROGRAM_ID, "owner"),
        ("discriminator", |a| a.data[0] ^= 0xff, "discriminator"),
        (
            "truncated",
            |a| {
                a.data.pop();
            },
            "length",
        ),
        ("extended", |a| a.data.push(0), "length"),
        ("executable", |a| a.executable = true, "executable"),
    ];
    for (_name, mutate, expected) in cases {
        let mut account = (*fixture.accounts[&POOL]).clone();
        mutate(&mut account);
        assert_error(
            EvolutAmm::from_keyed_account(
                &KeyedAccount {
                    key: POOL,
                    account,
                    params: None,
                },
                &fixture.context,
            ),
            expected,
        );
    }
}

#[test]
fn constructor_rejects_forged_pool_address_and_token_2022_pool() {
    let mut fixture = Fixture::unloaded();
    assert_error(
        EvolutAmm::from_keyed_account(
            &KeyedAccount {
                key: Pubkey::new_from_array([30; 32]),
                account: (*fixture.accounts[&POOL]).clone(),
                params: None,
            },
            &fixture.context,
        ),
        "pool PDA",
    );
    fixture.change_pool(|pool| pool.pool_type = 1);
    assert_error(fixture.amm.update(&fixture.accounts), "Token-2022");
    fixture.assert_unavailable();
}

#[test]
fn construction_alone_never_enables_quotes_or_instructions() {
    let fixture = Fixture::unloaded();
    assert_eq!(fixture.amm.get_reserve_mints(), vec![BASE, WSOL_MINT]);
    assert_eq!(fixture.amm.get_accounts_to_update().len(), 6);
    assert!(!fixture.amm.requires_update_for_reserve_mints());
    fixture.assert_unavailable();
}

#[test]
fn missing_dependency_invalidates_previous_quote_until_successful_refresh() {
    let dependencies = Fixture::loaded().amm.get_accounts_to_update();
    for key in dependencies {
        let mut fixture = Fixture::loaded();
        let removed = fixture.accounts.remove(&key).unwrap();
        assert!(
            fixture.amm.update(&fixture.accounts).is_err(),
            "missing {key}"
        );
        fixture.assert_unavailable();
        fixture.accounts.insert(key, removed);
        fixture.amm.update(&fixture.accounts).unwrap();
        assert!(fixture.amm.is_active());
        assert!(fixture.amm.quote(&buy(1_000_000)).unwrap().out_amount > 0);
    }
}

#[test]
fn corrupt_config_refresh_invalidates_previously_valid_state() {
    let cases: &[fn(&mut Account)] = &[
        |a| a.owner = TOKEN_PROGRAM_ID,
        |a| a.data[7] ^= 0xff,
        |a| {
            a.data.pop();
        },
        |a| a.data.push(0),
        |a| a.executable = true,
    ];
    for mutate in cases {
        let mut fixture = Fixture::loaded();
        let key = fixture.config_key();
        assert_eq!(fixture.accounts[&key].data.len(), CONFIG_ACCOUNT_LEN);
        mutate(fixture.accounts.get_mut(&key).unwrap());
        assert!(fixture.amm.update(&fixture.accounts).is_err());
        fixture.assert_unavailable();
    }
}

#[test]
fn unsupported_config_modes_fail_closed() {
    let cases: &[(&str, fn(&mut PoolConfig), &str)] = &[
        ("token2022", |c| c.token_type = 1, "Token-2022"),
        ("quote token2022", |c| c.quote_token_flag = 1, "Token-2022"),
        ("base fees", |c| c.collect_fee_mode = 1, "quote-token fees"),
        (
            "dynamic fees",
            |c| c.pool_fees.dynamic_fee.initialized = 1,
            "dynamic fees",
        ),
        (
            "scheduler",
            |c| c.pool_fees.base_fee.first_factor = 1,
            "flat fees",
        ),
        (
            "period",
            |c| c.pool_fees.base_fee.second_factor = 1,
            "flat fees",
        ),
        (
            "decay",
            |c| c.pool_fees.base_fee.third_factor = 1,
            "flat fees",
        ),
        (
            "rate limiter",
            |c| c.pool_fees.base_fee.base_fee_mode = 2,
            "flat fees",
        ),
        (
            "fee tier",
            |c| c.pool_fees.base_fee.cliff_fee_numerator = 30_000_000,
            "fee tier",
        ),
        (
            "creator share",
            |c| c.creator_trading_fee_percentage = 49,
            "creator fee share",
        ),
        ("version", |c| c.version = 3, "version"),
        ("activation", |c| c.activation_type = 2, "activation type"),
        (
            "quote mint",
            |c| c.quote_mint = anchor_lang::prelude::Pubkey::default(),
            "WSOL",
        ),
    ];
    for (_name, mutate, expected) in cases {
        let mut fixture = Fixture::loaded();
        fixture.change_config(mutate);
        assert_error(fixture.amm.update(&fixture.accounts), expected);
        fixture.assert_unavailable();
    }
}

#[test]
fn malformed_curve_limits_and_point_order_are_rejected() {
    let cases: &[fn(&mut PoolConfig)] = &[
        |c| c.sqrt_start_price = 0,
        |c| c.migration_quote_threshold = 0,
        |c| c.migration_sqrt_price = c.sqrt_start_price,
        |c| c.curve[0].sqrt_price = c.sqrt_start_price,
        |c| c.curve.iter_mut().for_each(|point| point.liquidity = 0),
    ];
    for mutate in cases {
        let mut fixture = Fixture::loaded();
        fixture.change_config(mutate);
        assert!(fixture.amm.update(&fixture.accounts).is_err());
        fixture.assert_unavailable();
    }
}

#[test]
fn migrated_completed_and_migrating_pools_are_unavailable() {
    for state in 0..3 {
        let mut fixture = Fixture::loaded();
        let threshold = fixture.config().migration_quote_threshold;
        fixture.change_pool(|pool| match state {
            0 => pool.is_migrated = 1,
            1 => pool.migration_progress = 1,
            _ => pool.quote_reserve = threshold,
        });
        // Keep the vault internally funded so rejection tests lifecycle state,
        // rather than failing the earlier reserve coverage check.
        let pool = fixture.pool();
        let quote_vault = fixture
            .accounts
            .get_mut(&contract_key(pool.quote_vault))
            .unwrap();
        let funded = pool.quote_reserve
            + pool.protocol_quote_fee
            + pool.partner_quote_fee
            + pool.creator_quote_fee;
        quote_vault.data[64..72].copy_from_slice(&funded.to_le_bytes());
        fixture.amm.update(&fixture.accounts).unwrap();
        assert_error(fixture.amm.quote(&buy(1_000_000)), "completed or migrated");
        fixture.assert_unavailable();
    }
}

#[test]
fn slot_and_timestamp_activation_are_rechecked_on_every_quote() {
    for activation_type in [0, 1] {
        let mut fixture = Fixture::loaded();
        fixture.change_config(|config| config.activation_type = activation_type);
        let now = if activation_type == 0 {
            fixture.context.clock_ref.slot.load(Ordering::Relaxed)
        } else {
            fixture
                .context
                .clock_ref
                .unix_timestamp
                .load(Ordering::Relaxed) as u64
        };
        fixture.change_pool(|pool| pool.activation_point = now + 1);
        fixture.amm.update(&fixture.accounts).unwrap();
        assert_error(fixture.amm.quote(&buy(1_000_000)), "not activated");
        fixture.assert_unavailable();
        if activation_type == 0 {
            fixture
                .context
                .clock_ref
                .slot
                .store(now + 1, Ordering::Relaxed);
        } else {
            fixture
                .context
                .clock_ref
                .unix_timestamp
                .store((now + 1) as i64, Ordering::Relaxed);
        }
        assert!(fixture.amm.is_active(), "activation is inclusive");
        assert!(fixture.amm.quote(&buy(1_000_000)).is_ok());
    }
}

#[test]
fn invalid_clock_never_produces_a_quote() {
    let fixture = Fixture::loaded();
    fixture
        .context
        .clock_ref
        .unix_timestamp
        .store(-1, Ordering::Relaxed);
    assert_error(
        fixture.amm.quote(&buy(1_000_000)),
        "negative clock timestamp",
    );
    fixture.assert_unavailable();
}

#[test]
fn exact_out_zero_input_and_wrong_mint_pairs_are_rejected() {
    let fixture = Fixture::loaded();
    assert!(!fixture.amm.supports_exact_out());
    let mut quote = buy(1_000_000);
    quote.swap_mode = SwapMode::ExactOut;
    assert_error(fixture.amm.quote(&quote), "ExactOut");
    let mut params = swap();
    params.swap_mode = SwapMode::ExactOut;
    assert!(fixture.amm.get_swap_and_account_metas(&params).is_err());
    assert!(fixture.amm.quote(&buy(0)).is_err());
    params.swap_mode = SwapMode::ExactIn;
    params.in_amount = 0;
    assert!(fixture.amm.get_swap_and_account_metas(&params).is_err());
    let other = Pubkey::new_from_array([33; 32]);
    for (input, output) in [
        (BASE, BASE),
        (WSOL_MINT, WSOL_MINT),
        (other, BASE),
        (BASE, other),
    ] {
        let mut quote = buy(1_000_000);
        quote.input_mint = input;
        quote.output_mint = output;
        assert_error(fixture.amm.quote(&quote), "mint pair");
        let mut params = swap();
        params.source_mint = input;
        params.destination_mint = output;
        assert_error(fixture.amm.get_swap_and_account_metas(&params), "mint pair");
    }
}

#[test]
fn cpi_transfer_authority_can_differ_from_user_and_payer() {
    let fixture = Fixture::loaded();
    let params = swap();
    assert_ne!(params.token_transfer_authority, params.user);
    assert_ne!(params.token_transfer_authority, params.payer);
    let result = fixture.amm.get_swap_and_account_metas(&params).unwrap();
    assert!(matches!(result.swap, Swap::Placeholder { data: None }));
    let pool = fixture.pool();
    assert_eq!(
        result.account_metas,
        vec![
            AccountMeta::new_readonly(pool_authority(), false),
            AccountMeta::new_readonly(contract_key(pool.config), false),
            AccountMeta::new(POOL, false),
            AccountMeta::new(params.source_token_account, false),
            AccountMeta::new(params.destination_token_account, false),
            AccountMeta::new(contract_key(pool.base_vault), false),
            AccountMeta::new(contract_key(pool.quote_vault), false),
            AccountMeta::new_readonly(BASE, false),
            AccountMeta::new_readonly(WSOL_MINT, false),
            AccountMeta::new_readonly(params.token_transfer_authority, true),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
            AccountMeta::new_readonly(event_authority(), false),
            AccountMeta::new_readonly(PROGRAM_ID, false),
        ]
    );
    assert_eq!(fixture.amm.get_accounts_len(), result.account_metas.len());
    let instruction = fixture.amm.native_swap_instruction(&params).unwrap();
    assert_eq!(instruction.program_id, PROGRAM_ID);
    assert_eq!(instruction.accounts, result.account_metas);
    assert_eq!(
        instruction.data,
        encode_native_swap(params.in_amount, params.out_amount)
    );
}

#[test]
fn native_swap_encoding_preserves_raw_u64_amounts_and_minimum_output() {
    let amount = u64::MAX - 7;
    let minimum = 0x0102_0304_0506_0708;
    let data = encode_native_swap(amount, minimum);
    assert_eq!(data.len(), 24);
    assert_eq!(&data[..8], &SWAP_DISCRIMINATOR);
    assert_eq!(&data[8..16], &amount.to_le_bytes());
    assert_eq!(&data[16..24], &minimum.to_le_bytes());
}

#[test]
fn nonempty_referrals_and_missing_or_duplicate_token_accounts_are_rejected() {
    let fixture = Fixture::loaded();
    let mut referrals = QuoteMintToReferrer::default();
    let mut params = swap();
    params.quote_mint_to_referrer = Some(&referrals);
    assert!(fixture.amm.get_swap_and_account_metas(&params).is_ok());
    referrals.insert(WSOL_MINT, Pubkey::new_from_array([48; 32]));
    let mut params = swap();
    params.quote_mint_to_referrer = Some(&referrals);
    assert_error(fixture.amm.get_swap_and_account_metas(&params), "referrals");
    for field in 0..4 {
        let mut params = swap();
        match field {
            0 => params.token_transfer_authority = Pubkey::default(),
            1 => params.source_token_account = Pubkey::default(),
            2 => params.destination_token_account = Pubkey::default(),
            _ => params.destination_token_account = params.source_token_account,
        }
        assert!(fixture.amm.get_swap_and_account_metas(&params).is_err());
    }
}

#[test]
fn legacy_mint_and_vault_integrity_are_required() {
    let mint_cases: &[fn(&mut Account)] = &[
        |a| a.owner = PROGRAM_ID,
        |a| a.executable = true,
        |a| a.data.push(0),
        |a| a.data[44] = a.data[44].wrapping_add(1),
        |a| a.data[45] = 0,
        |a| a.data[46..50].copy_from_slice(&1u32.to_le_bytes()),
    ];
    for mint in [BASE, WSOL_MINT] {
        for mutate in mint_cases {
            let mut fixture = Fixture::loaded();
            mutate(fixture.accounts.get_mut(&mint).unwrap());
            assert!(fixture.amm.update(&fixture.accounts).is_err());
            fixture.assert_unavailable();
        }
    }
    let vault_cases: &[fn(&mut Account)] = &[
        |a| a.owner = PROGRAM_ID,
        |a| a.executable = true,
        |a| a.data.push(0),
        |a| a.data[0] ^= 0xff,
        |a| a.data[32] ^= 0xff,
        |a| a.data[108] = 2,
        |a| a.data[72..76].copy_from_slice(&1u32.to_le_bytes()),
        |a| a.data[129..133].copy_from_slice(&1u32.to_le_bytes()),
        |a| a.data[109] ^= 1,
        |a| a.data[64..72].copy_from_slice(&0u64.to_le_bytes()),
    ];
    for is_quote in [false, true] {
        for mutate in vault_cases {
            let mut fixture = Fixture::loaded();
            let pool = fixture.pool();
            let vault = contract_key(if is_quote {
                pool.quote_vault
            } else {
                pool.base_vault
            });
            mutate(fixture.accounts.get_mut(&vault).unwrap());
            assert!(fixture.amm.update(&fixture.accounts).is_err());
            fixture.assert_unavailable();
        }
    }
}

#[test]
fn buy_fees_use_integer_ceiling_and_are_reported_in_wsol_for_both_tiers() {
    for numerator in [10_000_000u64, 20_000_000] {
        let mut fixture = Fixture::loaded();
        fixture.change_config(|config| config.pool_fees.base_fee.cliff_fee_numerator = numerator);
        fixture.amm.update(&fixture.accounts).unwrap();
        for amount in [51u64, 1_000_001, 25_000_007, 100_000_049] {
            let quote = fixture.amm.quote(&buy(amount)).unwrap();
            let expected_fee =
                ((u128::from(amount) * u128::from(numerator) + 999_999_999) / 1_000_000_000) as u64;
            assert_eq!(quote.in_amount, amount);
            assert!(quote.out_amount > 0);
            assert_eq!(
                quote.fee_amount, expected_fee,
                "amount {amount}, tier {numerator}"
            );
            assert_eq!(quote.fee_mint, WSOL_MINT);
            assert_eq!(
                quote.fee_pct,
                Decimal::from(numerator) / Decimal::from(1_000_000_000u64)
            );
            // Quotes must be read-only and repeat identically without a refresh.
            let repeated = fixture.amm.quote(&buy(amount)).unwrap();
            assert_eq!(repeated.out_amount, quote.out_amount);
            assert_eq!(repeated.fee_amount, quote.fee_amount);
        }
    }
}

#[test]
fn threshold_crossing_buy_is_rejected_instead_of_returning_a_partial_fill() {
    let fixture = Fixture::loaded();
    let amount = fixture
        .config()
        .migration_quote_threshold
        .checked_mul(2)
        .unwrap();
    assert!(fixture.amm.quote(&buy(amount)).is_err());
    // A rejected request does not mutate an otherwise valid pool snapshot.
    assert!(fixture.amm.quote(&buy(1_000_000)).unwrap().out_amount > 0);
}
