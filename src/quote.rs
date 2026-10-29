use super::{EvolutAmm, Result, WSOL_MINT, check};
use dynamic_bonding_curve::{params::swap::TradeDirection, state::fee::FeeMode};
use jupiter_amm_interface::{AmmError, Quote, QuoteParams, SwapMode};
use rust_decimal::Decimal;

pub(super) fn exact_in(amm: &EvolutAmm, params: &QuoteParams) -> Result<Quote> {
    check(
        params.swap_mode == SwapMode::ExactIn,
        "ExactOut is unsupported",
    )?;
    check(params.amount > 0, "input amount must be positive")?;
    let config = amm.require_active()?;
    let sell = amm.direction(params.input_mint, params.output_mint)?;
    let direction = if sell {
        TradeDirection::BaseToQuote
    } else {
        TradeDirection::QuoteToBase
    };
    let (timestamp, _, point) = amm.current_time()?;
    let mode =
        FeeMode::get_fee_mode(config.collect_fee_mode, direction, false).map_err(contract_error)?;
    let mut pool = amm.pool;
    // Mirror on-chain pre-swap and reserve checks using the exact vendored code.
    // The native exact-input method rejects any threshold-crossing partial fill.
    pool.update_pre_swap(config, timestamp)
        .map_err(contract_error)?;
    let result = pool
        .get_swap_result_from_exact_input(config, params.amount, &mode, direction, point)
        .map_err(contract_error)?;
    check(
        result.included_fee_input_amount == params.amount && result.amount_left == 0,
        "partial fills are unsupported",
    )?;
    check(
        result.output_amount > 0,
        "input is too small to produce output",
    )?;
    pool.apply_swap_result(
        config,
        &result.get_swap_result(),
        &mode,
        direction,
        timestamp,
    )
    .map_err(contract_error)?;
    let output_balance = if sell {
        amm.quote_vault_amount
    } else {
        amm.base_vault_amount
    };
    check(
        output_balance >= result.output_amount,
        "insufficient output vault balance",
    )?;
    if pool.is_curve_complete(config.migration_quote_threshold) {
        let base_balance = if sell {
            amm.base_vault_amount.checked_add(params.amount)
        } else {
            amm.base_vault_amount.checked_sub(result.output_amount)
        }
        .ok_or_else(|| AmmError::from("base vault arithmetic overflow"))?;
        let fees = pool
            .get_protocol_and_trading_base_fee()
            .map_err(contract_error)?;
        let vesting = config
            .locked_vesting_config
            .to_locked_vesting_params()
            .get_total_amount()
            .map_err(contract_error)?;
        let required = config
            .migration_base_threshold
            .checked_add(fees)
            .and_then(|v| v.checked_add(vesting))
            .ok_or_else(|| AmmError::from("migration balance arithmetic overflow"))?;
        check(
            base_balance >= required,
            "insufficient base liquidity for migration",
        )?;
    }
    let fee = result
        .trading_fee
        .checked_add(result.protocol_fee)
        .and_then(|v| v.checked_add(result.referral_fee))
        .ok_or_else(|| AmmError::from("fee arithmetic overflow"))?;
    Ok(Quote {
        in_amount: result.included_fee_input_amount,
        out_amount: result.output_amount,
        fee_amount: fee,
        fee_mint: WSOL_MINT,
        fee_pct: Decimal::from(config.pool_fees.base_fee.cliff_fee_numerator)
            / Decimal::from(1_000_000_000u64),
    })
}

fn contract_error(error: anchor_lang::error::Error) -> AmmError {
    AmmError::from(format!("Evolut contract quote: {error}"))
}
