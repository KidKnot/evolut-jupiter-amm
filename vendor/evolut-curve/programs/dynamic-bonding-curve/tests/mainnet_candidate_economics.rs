//! Offline candidate design using the actual contract's integer math and swap
//! implementation. No RPC, signatures, keys, or deployment. Not an SBF/CPI proof.
use dynamic_bonding_curve::{
    instructions::partner::ix_create_config::{ConfigParameters, LockedVestingParams, TokenSupplyParams},
    params::{fee_parameters::{BaseFeeParameters, PoolFeeParameters},
        liquidity_distribution::{get_base_token_for_swap, get_migration_base_token,
            get_migration_threshold_price, LiquidityDistributionParameters}, swap::TradeDirection},
    state::{fee::FeeMode, PoolConfig, VirtualPool},
};
use ruint::aliases::U256;

const SUPPLY: u64 = 1_000_000_000_000_000; // 1 billion, six decimals.
// Observed config #2 + fresh account rents at mainnet slot 458883349.
// This is an explicit scenario, not a live cost quote or an approved expense cap.
const OBSERVED_SETUP: u64 = 191_269_920;

fn sqrt_floor(value: U256) -> u128 {
    let (mut low, mut high) = (0u128, u128::MAX);
    while low < high {
        let mid = low + (high - low) / 2 + 1;
        if U256::from(mid) * U256::from(mid) <= value { low = mid; }
        else { high = mid - 1; }
    }
    low
}

fn candidate(threshold: u64) -> (ConfigParameters, PoolConfig) {
    candidate_with_fee(threshold, 10_000_000)
}

fn candidate_with_fee(threshold: u64, fee_numerator: u64) -> (ConfigParameters, PoolConfig) {
    // Proposed simple one-band curve: final/start sqrt ratio ~4, so ~80%
    // sold and ~20% reserved BEFORE setup expenses, with ~16x price movement.
    // p_start = T * (r+1) / (S*r*r), in lamports per base unit, Q128.
    let target = ((U256::from(threshold) * U256::from(5)) << 128usize)
        / (U256::from(SUPPLY) * U256::from(16));
    let mut start = sqrt_floor(target) + 1;
    for _ in 0..10_000 {
        let delta = 3 * start;
        let liquidity: u128 = (U256::from(threshold) << 128usize).div_ceil(U256::from(delta)).try_into().unwrap();
        // Tiny integer headroom ensures the terminal quote target is inside
        // the band, while limiting excess inventory to rounding dust.
        let curve = vec![LiquidityDistributionParameters { sqrt_price: 4 * start + 2, liquidity }];
        let end = get_migration_threshold_price(threshold, start, &curve).unwrap();
        let sold: u64 = get_base_token_for_swap(start, end, &curve).unwrap().try_into().unwrap();
        let reserve = get_migration_base_token(threshold, end).unwrap();
        let buffered = PoolConfig::get_swap_amount_with_buffer(sold, start, &curve).unwrap();
        if buffered + reserve > SUPPLY { start += 1; continue; }
        let post_supply_floor = if cfg!(feature = "mainnet") { 0 } else { sold + reserve };
        let parameters = ConfigParameters {
            pool_fees: PoolFeeParameters { base_fee: BaseFeeParameters {
                cliff_fee_numerator: fee_numerator, ..Default::default()
            }, dynamic_fee: None },
            collect_fee_mode: 0, activation_type: 0, token_decimal: 6,
            migration_quote_threshold: threshold, sqrt_start_price: start,
            locked_vesting: LockedVestingParams::default(),
            token_supply: Some(TokenSupplyParams { pre_migration_token_supply: SUPPLY,
                post_migration_token_supply: post_supply_floor }),
            token_update_authority: 2,
            // Paid-launch platform revenue is a separate payment; do not
            // accidentally charge another 0.015 SOL through this field.
            pool_creation_fee: 0, max_setup_lamports: OBSERVED_SETUP,
            curve,
        };
        parameters.validate().unwrap();
        parameters.token_supply.unwrap().validate_supply_requirements(buffered + reserve, sold + reserve).unwrap();
        let mut config = PoolConfig::default();
        config.version = if cfg!(feature = "mainnet") { 2 } else { 1 };
        if cfg!(feature = "mainnet") {
            config._padding[..8].copy_from_slice(&PoolConfig::mainnet_graduation_fee(threshold).unwrap().to_le_bytes());
        }
        config.max_setup_lamports = OBSERVED_SETUP;
        config.pool_fees = parameters.pool_fees.to_pool_fees_config();
        config.sqrt_start_price = start;
        config.migration_sqrt_price = end;
        config.migration_quote_threshold = threshold;
        config.swap_base_amount = sold;
        config.migration_base_threshold = reserve;
        config.pre_migration_token_supply = SUPPLY;
        config.post_migration_token_supply = post_supply_floor;
        config.fixed_token_supply_flag = 1;
        config.creator_trading_fee_percentage = 50;
        config.token_decimal = 6;
        config.curve[0] = parameters.curve[0].to_liquidity_distribution_config();
        assert_eq!(config.get_initial_base_supply().unwrap(), SUPPLY);
        return (parameters, config);
    }
    panic!("No valid candidate within bounded rounding adjustment");
}

#[test]
fn both_curve_tiers_charge_exact_quote_fees_and_keep_equal_rewards() {
    for threshold in [500_000_000u64, 85_000_000_000] {
        for numerator in [10_000_000u64, 20_000_000] {
            let (_, config) = candidate_with_fee(threshold, numerator);
            for gross in [10_001u64, 100_000_001, threshold / 3] {
                let mut pool = VirtualPool { sqrt_price: config.sqrt_start_price,
                    base_reserve: SUPPLY, ..Default::default() };
                let buy_mode = FeeMode::get_fee_mode(0, TradeDirection::QuoteToBase, false).unwrap();
                let buy = pool.get_swap_result_from_partial_input(&config, gross, &buy_mode, TradeDirection::QuoteToBase, 0).unwrap();
                let expected_buy_fee = ((u128::from(gross) * u128::from(numerator) + 999_999_999) / 1_000_000_000) as u64;
                assert_eq!(buy.trading_fee, expected_buy_fee);
                assert_eq!(buy.protocol_fee + buy.referral_fee, 0);
                pool.apply_swap_result(&config, &buy.get_swap_result(), &buy_mode, TradeDirection::QuoteToBase, 0).unwrap();
                assert_eq!(pool.creator_quote_fee, expected_buy_fee / 2);
                assert_eq!(pool.partner_quote_fee, expected_buy_fee - expected_buy_fee / 2);
                assert_eq!(pool.quote_reserve + expected_buy_fee, gross);
                let sell_mode = FeeMode::get_fee_mode(0, TradeDirection::BaseToQuote, false).unwrap();
                let sell = pool.get_swap_result_from_partial_input(&config, buy.output_amount, &sell_mode, TradeDirection::BaseToQuote, 0).unwrap();
                let gross_out = sell.output_amount + sell.trading_fee;
                let expected_sell_fee = ((u128::from(gross_out) * u128::from(numerator) + 999_999_999) / 1_000_000_000) as u64;
                assert_eq!(sell.trading_fee, expected_sell_fee);
                assert_eq!(sell.protocol_fee + sell.referral_fee, 0);
                pool.apply_swap_result(&config, &sell.get_swap_result(), &sell_mode, TradeDirection::BaseToQuote, 0).unwrap();
                assert_eq!(pool.creator_quote_fee, expected_buy_fee / 2 + expected_sell_fee / 2);
                assert_eq!(pool.partner_quote_fee + pool.creator_quote_fee, expected_buy_fee + expected_sell_fee);
                assert_eq!(pool.base_reserve, SUPPLY);
                assert_eq!(gross, sell.output_amount + pool.quote_reserve + pool.partner_quote_fee + pool.creator_quote_fee);
            }
        }
    }
}

#[test]
fn both_curve_tiers_partial_fill_only_consumed_sol_at_both_graduation_thresholds() {
    for threshold in [500_000_000u64, 85_000_000_000] {
        for numerator in [10_000_000u64, 20_000_000] {
            let (_, config) = candidate_with_fee(threshold, numerator);
            let mut pool = VirtualPool { sqrt_price: config.sqrt_start_price,
                base_reserve: SUPPLY, ..Default::default() };
            let mode = FeeMode::get_fee_mode(0, TradeDirection::QuoteToBase, false).unwrap();
            let requested = threshold * 2;
            let swap = pool.get_swap_result_from_partial_input(&config, requested, &mode, TradeDirection::QuoteToBase, 0).unwrap();
            let consumed = swap.included_fee_input_amount;
            let net = swap.excluded_fee_input_amount;
            assert!(consumed < requested);
            assert_eq!(consumed, net + swap.trading_fee);
            let expected_consumed = ((u128::from(net) * 1_000_000_000 + u128::from(1_000_000_000 - numerator) - 1) / u128::from(1_000_000_000 - numerator)) as u64;
            assert_eq!(consumed, expected_consumed);
            pool.apply_swap_result(&config, &swap.get_swap_result(), &mode, TradeDirection::QuoteToBase, 0).unwrap();
            assert!((threshold..=threshold + 2).contains(&pool.quote_reserve));
            assert_eq!(pool.creator_quote_fee, swap.trading_fee / 2);
            assert_eq!(pool.partner_quote_fee + pool.creator_quote_fee, swap.trading_fee);
            assert!(pool.is_curve_complete(threshold));
            let quote_deposit = config.graduation_quote_deposit(OBSERVED_SETUP).unwrap();
            assert_eq!(quote_deposit + OBSERVED_SETUP + config.graduation_fee_lamports().unwrap(), threshold);
        }
    }
}

fn trade(pool: &mut VirtualPool, config: &PoolConfig, gross: u64, direction: TradeDirection) -> u64 {
    let mode = FeeMode::get_fee_mode(0, direction, false).unwrap();
    let swap = pool.get_swap_result_from_partial_input(config, gross, &mode, direction, 0).unwrap();
    assert_eq!(swap.protocol_fee, 0);
    assert_eq!(swap.referral_fee, 0);
    pool.apply_swap_result(config, &swap.get_swap_result(), &mode, direction, 0).unwrap();
    assert_eq!(pool.partner_base_fee + pool.creator_base_fee, 0);
    swap.output_amount
}

#[test]
fn both_profiles_reach_graduation_with_fixed_supply_and_quote_fee_conservation() {
    for threshold in [500_000_000u64, 85_000_000_000] {
        let (parameters, config) = candidate(threshold);
        let mut pool = VirtualPool { sqrt_price: config.sqrt_start_price,
            base_reserve: SUPPLY, ..Default::default() };
        let mut sold = 0;
        for _ in 0..101 {
            if pool.quote_reserve >= threshold { break; }
            sold += trade(&mut pool, &config, threshold / 99 + 100, TradeDirection::QuoteToBase);
        }
        // Partial-input swap rounding may leave one lamport of quote surplus;
        // graduation uses the threshold, not that surplus.
        assert!((threshold..=threshold + 2).contains(&pool.quote_reserve));
        assert!(pool.is_curve_complete(threshold));
        assert_eq!(sold + pool.base_reserve, SUPPLY);
        assert!(pool.partner_quote_fee.abs_diff(pool.creator_quote_fee) <= 101);
        assert!(pool.partner_quote_fee + pool.creator_quote_fee >= threshold / 99);
        let quote_deposit = config.graduation_quote_deposit(OBSERVED_SETUP).unwrap();
        assert_eq!(quote_deposit + OBSERVED_SETUP + config.graduation_fee_lamports().unwrap(), threshold);
        let deposit = get_migration_base_token(quote_deposit, config.migration_sqrt_price).unwrap();
        assert!(deposit > 0 && deposit <= pool.base_reserve);
        let unused = pool.base_reserve - deposit;
        let (burnable, unassigned) = config.graduation_burn_amounts(
            pool.base_reserve, 0, deposit, unused, 0,
        ).unwrap();
        if cfg!(feature = "mainnet") {
            assert_eq!(burnable, unused);
            assert_eq!(unassigned, 0);
            assert_eq!(SUPPLY - burnable, sold + deposit);
        } else {
            assert!(unassigned > 0, "legacy devnet supply-floor policy must remain unchanged");
        }
        let price = |sqrt: u128| (sqrt as f64 / 2f64.powi(64)).powi(2) / 1000.0;
        println!("CANDIDATE threshold={} start_sqrt={} end_sqrt={} band_end={} liquidity={} start_sol_per_token={:.14} end_sol_per_token={:.14} sold_base_units={} amm_base_units={} amm_lamports={} burned_base_units={} post_supply_base_units={} claimable_base_units={} READY=false",
            threshold, config.sqrt_start_price, config.migration_sqrt_price,
            parameters.curve[0].sqrt_price, parameters.curve[0].liquidity,
            price(config.sqrt_start_price), price(config.migration_sqrt_price),
            sold, deposit, quote_deposit, burnable, SUPPLY - burnable, unassigned);
    }
}

#[test]
fn buy_sell_round_trip_cannot_create_tokens_or_sol() {
    for threshold in [500_000_000u64, 85_000_000_000] {
        let (_, config) = candidate(threshold);
        for input in [10_000u64, threshold / 100, threshold / 10] {
            let mut pool = VirtualPool { sqrt_price: config.sqrt_start_price,
                base_reserve: SUPPLY, ..Default::default() };
            let tokens = trade(&mut pool, &config, input, TradeDirection::QuoteToBase);
            let returned = trade(&mut pool, &config, tokens, TradeDirection::BaseToQuote);
            assert!(returned < input);
            assert_eq!(pool.base_reserve, SUPPLY);
            assert_eq!(input, returned + pool.quote_reserve + pool.partner_quote_fee + pool.creator_quote_fee);
        }
    }
}

#[cfg(feature = "mainnet")]
#[test]
fn mainnet_new_config_rejects_supply_drift_extra_minting_vesting_and_double_launch_fee() {
    let (approved, _) = candidate(85_000_000_000);
    for decimals in [0, 5, 7, 9] {
        let mut p = approved.clone(); p.token_decimal = decimals;
        assert!(p.validate().is_err());
    }
    for authority in [1, 3, 4, 255] {
        let mut p = approved.clone(); p.token_update_authority = authority;
        assert!(p.validate().is_err());
    }
    for supply in [None, Some(TokenSupplyParams { pre_migration_token_supply: SUPPLY + 1,
        post_migration_token_supply: SUPPLY })] {
        let mut p = approved.clone(); p.token_supply = supply;
        assert!(p.validate().is_err());
    }
    let mut p = approved.clone(); p.pool_creation_fee = 15_000_000;
    assert!(p.validate().is_err());
    let mut p = approved.clone(); p.token_supply.as_mut().unwrap().post_migration_token_supply = 1;
    assert!(p.validate().is_err());
    let mut p = approved.clone(); p.locked_vesting.cliff_unlock_amount = 1;
    assert!(p.validate().is_err());
    let mut p = approved; p.token_update_authority = 0;
    assert!(p.validate().is_ok());
}
