//! Read-only initial-buy quote. User prefunding and retries are backend-managed;
//! the mainnet platform fee is enforced atomically by initialization.
//! No on-chain payment ticket, escrow, refund or launch-state machinery.
use crate::{
    params::swap::TradeDirection,
    state::{
        fee::{FeeMode, VolatilityTracker},
        PoolConfig, PoolType, TokenType, VirtualPool,
    },
    PoolError,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct QuotePaidLaunchBuyCtx<'info> {
    pub config: AccountLoader<'info, PoolConfig>,
}

// Quote against a freshly initialized pool using exactly the program's own math.
// This instruction is read-only and can be simulated before any payment or AI.
pub fn handle_quote_paid_launch_buy(
    ctx: Context<QuotePaidLaunchBuyCtx>,
    amount: u64,
) -> Result<()> {
    let config = ctx.accounts.config.load()?;
    require!(
        config.token_type == TokenType::SplToken as u8 && amount > 0 && amount <= 100_000_000_000,
        PoolError::InvalidInput
    );
    let current = crate::activation_handler::get_current_point(config.activation_type)?;
    let mut pool = VirtualPool::default();
    pool.initialize(
        VolatilityTracker::default(),
        ctx.accounts.config.key(),
        Pubkey::default(),
        Pubkey::default(),
        Pubkey::default(),
        Pubkey::default(),
        config.sqrt_start_price,
        PoolType::SplToken.into(),
        current,
        config.get_initial_base_supply()?,
    );
    let direction = TradeDirection::QuoteToBase;
    let fees = FeeMode::get_fee_mode(config.collect_fee_mode, direction, false)?;
    let quote =
        pool.get_swap_result_from_partial_input(&config, amount, &fees, direction, current)?;
    let mut result = [0u8; 16];
    result[..8].copy_from_slice(&quote.included_fee_input_amount.to_le_bytes());
    result[8..].copy_from_slice(&quote.output_amount.to_le_bytes());
    anchor_lang::solana_program::program::set_return_data(&result);
    Ok(())
}
