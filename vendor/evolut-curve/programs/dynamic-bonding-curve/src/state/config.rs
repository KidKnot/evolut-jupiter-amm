use anchor_lang::prelude::*;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ruint::aliases::U256;
use static_assertions::const_assert_eq;

use crate::{
    base_fee::{get_base_fee_handler, BaseFeeHandler, FeeRateLimiter},
    constants::{
        fee::{
            FEE_DENOMINATOR, HOST_FEE_PERCENT, MAX_FEE_NUMERATOR, PROTOCOL_FEE_PERCENT,
            PROTOCOL_POOL_CREATION_FEE_PERCENT,
        },
        MAX_CURVE_POINT_CONFIG, MAX_SQRT_PRICE, SWAP_BUFFER_PERCENTAGE,
    },
    params::{
        liquidity_distribution::{get_base_token_for_swap, LiquidityDistributionParameters},
        swap::TradeDirection,
    },
    safe_math::SafeMath,
    u128x128_math::Rounding,
    utils_math::safe_mul_div_cast_u64,
    LockedVestingParams, PoolError,
};

use super::fee::{FeeOnAmountResult, VolatilityTracker};

/// base fee mode
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    IntoPrimitive,
    TryFromPrimitive,
    AnchorDeserialize,
    AnchorSerialize,
)]
// https://www.desmos.com/calculator/oxdndn2xdx
pub enum BaseFeeMode {
    // fee = cliff_fee_numerator - passed_period * reduction_factor
    FeeSchedulerLinear,
    // fee = cliff_fee_numerator * (1-reduction_factor/10_000)^passed_period
    FeeSchedulerExponential,
    // TODO
    RateLimiter,
}

#[zero_copy]
#[derive(Debug, InitSpace, Default)]
pub struct PoolFeesConfig {
    pub base_fee: BaseFeeConfig,
    pub dynamic_fee: DynamicFeeConfig,
}

const_assert_eq!(PoolFeesConfig::INIT_SPACE, 80);

impl PoolFeesConfig {
    /// Calculates the total trading fee numerator by combining base fee and dynamic fee.
    /// The base fee is determined by the fee scheduler mode (linear or exponential) and time period.
    /// The dynamic fee is based on price volatility and is only applied if dynamic fees are enabled.
    /// The total fee is capped at MAX_FEE_NUMERATOR (99%) to ensure reasonable trading costs.
    ///
    /// Returns the total fee numerator that will be used to calculate actual trading fees.
    pub fn get_total_fee_numerator_from_included_fee_amount(
        &self,
        volatility_tracker: &VolatilityTracker,
        current_point: u64,
        activation_point: u64,
        included_fee_amount: u64,
        trade_direction: TradeDirection,
    ) -> Result<u64> {
        let base_fee_handler = self.base_fee.get_base_fee_handler()?;

        let base_fee_numerator = base_fee_handler.get_base_fee_numerator_from_included_fee_amount(
            current_point,
            activation_point,
            trade_direction,
            included_fee_amount,
        )?;

        self.get_total_fee_numerator(base_fee_numerator, volatility_tracker)
    }

    pub fn get_total_fee_numerator_from_excluded_fee_amount(
        &self,
        volatility_tracker: &VolatilityTracker,
        current_point: u64,
        activation_point: u64,
        excluded_fee_amount: u64,
        trade_direction: TradeDirection,
    ) -> Result<u64> {
        let base_fee_handler = self.base_fee.get_base_fee_handler()?;

        let base_fee_numerator = base_fee_handler.get_base_fee_numerator_from_excluded_fee_amount(
            current_point,
            activation_point,
            trade_direction,
            excluded_fee_amount,
        )?;

        self.get_total_fee_numerator(base_fee_numerator, volatility_tracker)
    }

    fn get_total_fee_numerator(
        &self,
        base_fee_numerator: u64,
        volatility_tracker: &VolatilityTracker,
    ) -> Result<u64> {
        let total_fee_numerator = self
            .dynamic_fee
            .get_variable_fee_numerator(volatility_tracker)?
            .safe_add(base_fee_numerator.into())?;

        // Cap the total fee at MAX_FEE_NUMERATOR
        let total_fee_numerator = if total_fee_numerator > MAX_FEE_NUMERATOR.into() {
            MAX_FEE_NUMERATOR
        } else {
            total_fee_numerator.try_into().unwrap()
        };

        Ok(total_fee_numerator)
    }

    pub fn get_fee_on_amount(
        &self,
        trade_fee_numerator: u64,
        amount: u64,
        has_referral: bool,
    ) -> Result<FeeOnAmountResult> {
        let (amount, trading_fee) =
            PoolFeesConfig::get_excluded_fee_amount(trade_fee_numerator, amount)?;

        let protocol_fee = safe_mul_div_cast_u64(
            trading_fee,
            PROTOCOL_FEE_PERCENT.into(),
            100,
            Rounding::Down,
        )?;

        // update trading fee
        let trading_fee: u64 = trading_fee.safe_sub(protocol_fee)?;

        let referral_fee = if has_referral {
            safe_mul_div_cast_u64(protocol_fee, HOST_FEE_PERCENT.into(), 100, Rounding::Down)?
        } else {
            0
        };

        let protocol_fee = protocol_fee.safe_sub(referral_fee)?;

        Ok(FeeOnAmountResult {
            amount,
            protocol_fee,
            referral_fee,
            trading_fee,
        })
    }

    pub fn get_excluded_fee_amount(
        trade_fee_numerator: u64,
        included_fee_amount: u64,
    ) -> Result<(u64, u64)> {
        let trading_fee: u64 = safe_mul_div_cast_u64(
            included_fee_amount,
            trade_fee_numerator,
            FEE_DENOMINATOR,
            Rounding::Up,
        )?;
        // update amount
        let excluded_fee_amount = included_fee_amount.safe_sub(trading_fee)?;
        Ok((excluded_fee_amount, trading_fee))
    }

    pub fn get_included_fee_amount(
        trade_fee_numerator: u64,
        excluded_fee_amount: u64,
    ) -> Result<(u64, u64)> {
        let included_fee_amount: u64 = safe_mul_div_cast_u64(
            excluded_fee_amount,
            FEE_DENOMINATOR,
            FEE_DENOMINATOR.safe_sub(trade_fee_numerator)?,
            Rounding::Up,
        )?;
        let fee_amount = included_fee_amount.safe_sub(excluded_fee_amount)?;
        Ok((included_fee_amount, fee_amount))
    }

    pub fn split_fees(&self, fee_amount: u64, has_referral: bool) -> Result<(u64, u64, u64)> {
        let protocol_fee =
            safe_mul_div_cast_u64(fee_amount, PROTOCOL_FEE_PERCENT.into(), 100, Rounding::Down)?;

        // update trading fee
        let trading_fee: u64 = fee_amount.safe_sub(protocol_fee)?;

        let referral_fee = if has_referral {
            safe_mul_div_cast_u64(protocol_fee, HOST_FEE_PERCENT.into(), 100, Rounding::Down)?
        } else {
            0
        };

        let protocol_fee = protocol_fee.safe_sub(referral_fee)?;

        Ok((trading_fee, protocol_fee, referral_fee))
    }
}

#[zero_copy]
#[derive(Debug, InitSpace, Default)]
pub struct BaseFeeConfig {
    pub cliff_fee_numerator: u64,
    // reverse order to ensure it is backward-compatible on fee scheduler
    // first_factor: number_of_period, period_frequency: second_factor, reduction_factor: third_factor
    pub second_factor: u64,
    pub third_factor: u64,
    pub first_factor: u16,
    pub base_fee_mode: u8,
    pub padding_0: [u8; 5],
}

const_assert_eq!(BaseFeeConfig::INIT_SPACE, 32);

impl BaseFeeConfig {
    pub fn get_fee_rate_limiter(&self) -> Result<FeeRateLimiter> {
        let base_fee_mode =
            BaseFeeMode::try_from(self.base_fee_mode).map_err(|_| PoolError::InvalidBaseFeeMode)?;
        if base_fee_mode == BaseFeeMode::RateLimiter {
            Ok(FeeRateLimiter {
                cliff_fee_numerator: self.cliff_fee_numerator,
                reference_amount: self.third_factor,
                max_limiter_duration: self.second_factor,
                fee_increment_bps: self.first_factor,
            })
        } else {
            Err(PoolError::InvalidFeeRateLimiter.into())
        }
    }

    pub fn get_base_fee_handler(&self) -> Result<Box<dyn BaseFeeHandler>> {
        get_base_fee_handler(
            self.cliff_fee_numerator,
            self.first_factor,
            self.second_factor,
            self.third_factor,
            self.base_fee_mode,
        )
    }

    pub fn is_fee_rate_limiter_applied(&self, trade_fee_numerator: u64) -> Result<bool> {
        let base_fee_mode =
            BaseFeeMode::try_from(self.base_fee_mode).map_err(|_| PoolError::InvalidBaseFeeMode)?;

        if base_fee_mode == BaseFeeMode::RateLimiter {
            return Ok(trade_fee_numerator > self.cliff_fee_numerator);
        }

        Ok(false)
    }

    pub fn validate_min_base_fee(&self) -> Result<()> {
        let base_fee_handler = self.get_base_fee_handler()?;
        base_fee_handler.validate_min_base_fee()?;

        Ok(())
    }
}

#[zero_copy]
#[derive(Debug, InitSpace, Default)]
pub struct DynamicFeeConfig {
    pub initialized: u8, // 0, ignore for dynamic fee
    pub padding: [u8; 7],
    pub max_volatility_accumulator: u32,
    pub variable_fee_control: u32,
    pub bin_step: u16,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub padding2: [u8; 8], // Add padding for u128 alignment
    pub bin_step_u128: u128,
}

const_assert_eq!(DynamicFeeConfig::INIT_SPACE, 48);

impl DynamicFeeConfig {
    pub fn is_dynamic_fee_enable(&self) -> bool {
        self.initialized != 0
    }

    pub fn get_variable_fee_numerator(
        &self,
        volatility_tracker: &VolatilityTracker,
    ) -> Result<u128> {
        if !self.is_dynamic_fee_enable() {
            return Ok(0);
        }

        // 1. Computing the squared price movement (volatility_accumulator * bin_step)^2
        let square_vfa_bin: u128 = volatility_tracker
            .volatility_accumulator
            .safe_mul(self.bin_step.into())?
            .checked_pow(2)
            .ok_or_else(|| PoolError::MathOverflow)?;

        // 2. Multiplying by the fee control factor
        let v_fee = square_vfa_bin.safe_mul(self.variable_fee_control.into())?;

        // 3. Scaling down the result to fit within u64 range (dividing by 1e11 and rounding up)
        let scaled_v_fee = v_fee.safe_add(99_999_999_999)?.safe_div(100_000_000_000)?;

        Ok(scaled_v_fee)
    }
}

#[zero_copy]
#[derive(Debug, InitSpace, Default)]
pub struct LockedVestingConfig {
    pub amount_per_period: u64,
    pub cliff_duration_from_migration_time: u64,
    pub frequency: u64,
    pub number_of_period: u64,
    pub cliff_unlock_amount: u64,
    pub _padding: u64,
}

const_assert_eq!(LockedVestingConfig::INIT_SPACE, 48);

impl LockedVestingConfig {
    pub fn to_locked_vesting_params(&self) -> LockedVestingParams {
        LockedVestingParams {
            amount_per_period: self.amount_per_period,
            cliff_duration_from_migration_time: self.cliff_duration_from_migration_time,
            frequency: self.frequency,
            number_of_period: self.number_of_period,
            cliff_unlock_amount: self.cliff_unlock_amount,
        }
    }
}

#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    IntoPrimitive,
    TryFromPrimitive,
    AnchorDeserialize,
    AnchorSerialize,
    Default,
)]
pub enum TokenAuthorityOption {
    // Creator has permission to update update_authority
    #[default]
    CreatorUpdateAuthority,
    // No one has permission to update the authority
    Immutable,
    // Partner has permission to update update_authority
    PartnerUpdateAuthority,
    // Creator has permission as mint_authority and update_authority
    CreatorUpdateAndMintAuthority,
    // Partner has permission as mint_authority and update_authority
    PartnerUpdateAndMintAuthority,
}

impl TokenAuthorityOption {
    pub fn get_update_authority(&self, creator: Pubkey, partner: Pubkey) -> Option<Pubkey> {
        match *self {
            TokenAuthorityOption::CreatorUpdateAndMintAuthority
            | TokenAuthorityOption::CreatorUpdateAuthority => Some(creator),

            TokenAuthorityOption::PartnerUpdateAndMintAuthority
            | TokenAuthorityOption::PartnerUpdateAuthority => Some(partner),
            TokenAuthorityOption::Immutable => None,
        }
    }

    pub fn get_mint_authority(&self, creator: Pubkey, partner: Pubkey) -> Option<Pubkey> {
        match *self {
            TokenAuthorityOption::CreatorUpdateAndMintAuthority => Some(creator),
            TokenAuthorityOption::PartnerUpdateAndMintAuthority => Some(partner),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    IntoPrimitive,
    TryFromPrimitive,
    AnchorDeserialize,
    AnchorSerialize,
)]
pub enum TokenType {
    SplToken,
    Token2022,
}

#[account(zero_copy)]
#[derive(InitSpace, Debug, Default)]
pub struct PoolConfig {
    pub quote_mint: Pubkey,
    pub fee_claimer: Pubkey,
    pub leftover_receiver: Pubkey,
    /// Immutable official Raydium fee configuration.
    pub raydium_amm_config: Pubkey,
    pub pool_fees: PoolFeesConfig,
    pub locked_vesting_config: LockedVestingConfig,
    pub migration_sqrt_price: u128,
    pub sqrt_start_price: u128,
    pub curve: [LiquidityDistributionConfig; MAX_CURVE_POINT_CONFIG],
    pub swap_base_amount: u64,
    pub migration_quote_threshold: u64,
    pub migration_base_threshold: u64,
    pub pre_migration_token_supply: u64,
    pub post_migration_token_supply: u64,
    pub pool_creation_fee: u64,
    pub max_setup_lamports: u64,
    pub raydium_trade_fee_rate: u64,
    pub raydium_protocol_fee_rate: u64,
    pub raydium_fund_fee_rate: u64,
    pub raydium_create_pool_fee: u64,
    pub collect_fee_mode: u8,
    pub activation_type: u8,
    pub token_decimal: u8,
    pub version: u8,
    pub token_type: u8,
    pub quote_token_flag: u8,
    pub fixed_token_supply_flag: u8,
    pub creator_trading_fee_percentage: u8,
    pub token_update_authority: u8,
    /// v1: all zero. v2: first eight bytes are the fixed graduation revenue
    /// in little-endian lamports; last seven remain reserved. Same account ABI.
    pub _padding: [u8; 15],
}
const_assert_eq!(PoolConfig::INIT_SPACE, 1040);

#[zero_copy]
#[derive(InitSpace, Debug, Default)]
pub struct LiquidityDistributionConfig {
    pub sqrt_price: u128,
    pub liquidity: u128,
}

impl LiquidityDistributionConfig {
    pub fn to_liquidity_distribution_parameters(&self) -> LiquidityDistributionParameters {
        LiquidityDistributionParameters {
            sqrt_price: self.sqrt_price,
            liquidity: self.liquidity,
        }
    }
}

impl PoolConfig {
    /// Owner-approved NEW mainnet profiles. Never infer a fee for legacy pools.
    pub fn mainnet_graduation_fee(threshold: u64) -> Result<u64> {
        match threshold {
            500_000_000 => Ok(0),
            85_000_000_000 => Ok(800_000_000),
            _ => err!(PoolError::InvalidQuoteThreshold),
        }
    }

    pub fn graduation_fee_lamports(&self) -> Result<u64> {
        match self.version {
            1 => {
                require!(self._padding == [0; 15], PoolError::InvalidInput);
                Ok(0)
            }
            2 => {
                require!(self._padding[8..] == [0; 7], PoolError::InvalidInput);
                let fee = u64::from_le_bytes(self._padding[..8].try_into().unwrap());
                require!(fee == Self::mainnet_graduation_fee(self.migration_quote_threshold)?,
                    PoolError::InvalidInput);
                Ok(fee)
            }
            _ => err!(PoolError::InvalidInput),
        }
    }

    pub fn graduation_quote_deposit(&self, actual_setup: u64) -> Result<u64> {
        require!(actual_setup <= self.max_setup_lamports, PoolError::InvalidInput);
        let deductions = actual_setup.safe_add(self.graduation_fee_lamports()?)?;
        require!(deductions < self.migration_quote_threshold, PoolError::InvalidQuoteThreshold);
        Ok(self.migration_quote_threshold.safe_sub(deductions)?)
    }

    pub fn get_token_authority(&self) -> Result<TokenAuthorityOption> {
        let token_authority = TokenAuthorityOption::try_from(self.token_update_authority)
            .map_err(|_| PoolError::InvalidTokenAuthorityOption)?;
        Ok(token_authority)
    }

    pub fn get_swap_amount_with_buffer(
        swap_base_amount: u64,
        sqrt_start_price: u128,
        curve: &[LiquidityDistributionParameters],
    ) -> Result<u64> {
        let swap_amount_buffer = u128::from(swap_base_amount)
            .safe_mul(SWAP_BUFFER_PERCENTAGE.into())?
            .safe_div(100)?
            .safe_add(swap_base_amount.into())?;
        let max_base_amount_on_curve =
            get_base_token_for_swap(sqrt_start_price, MAX_SQRT_PRICE, &curve)?;

        if U256::from(swap_amount_buffer) < max_base_amount_on_curve {
            Ok(u64::try_from(swap_amount_buffer).map_err(|_| PoolError::MathOverflow)?)
        } else {
            Ok(max_base_amount_on_curve
                .try_into()
                .map_err(|_| PoolError::MathOverflow)?)
        }
    }
    pub fn get_total_token_supply(
        swap_base_amount: u64,
        migration_base_threshold: u64,
        locked_vesting_params: &LockedVestingParams,
    ) -> Result<u64> {
        let total_circulating_amount = swap_base_amount.safe_add(migration_base_threshold)?;
        let total_locked_vesting_amount = locked_vesting_params.get_total_amount()?;
        let total_amount = total_circulating_amount.safe_add(total_locked_vesting_amount)?;
        Ok(total_amount)
    }

    pub fn get_initial_base_supply(&self) -> Result<u64> {
        if self.is_fixed_token_supply() {
            Ok(self.pre_migration_token_supply)
        } else {
            let mut curve = vec![];
            for i in 0..MAX_CURVE_POINT_CONFIG {
                if self.curve[i].liquidity == 0 {
                    break;
                }
                curve.push(self.curve[i].to_liquidity_distribution_parameters());
            }
            let swap_amount_with_buffer = PoolConfig::get_swap_amount_with_buffer(
                self.swap_base_amount,
                self.sqrt_start_price,
                &curve,
            )?;
            PoolConfig::get_total_token_supply(
                swap_amount_with_buffer,
                self.migration_base_threshold,
                &self.locked_vesting_config.to_locked_vesting_params(),
            )
        }
    }

    fn get_max_burnable_amount_post_migration(&self) -> Result<u64> {
        if self.is_fixed_token_supply() {
            Ok(self
                .pre_migration_token_supply
                .safe_sub(self.post_migration_token_supply)?)
        } else {
            Ok(u64::MAX)
        }
    }

    /// leftover is extra base token in base vault after curve is completed
    pub fn get_burnable_amount_post_migration(&self, leftover: u64) -> Result<u64> {
        let max_burnable_amount = self.get_max_burnable_amount_post_migration()?;
        Ok(max_burnable_amount.min(leftover))
    }

    /// Only issued, unused inventory is eligible. Vault balances may also
    /// contain protected trading fees and unsolicited donations, neither of
    /// which belongs to the graduation burn budget.
    pub fn graduation_burn_amounts(
        &self, base_reserve: u64, vested: u64, deposited: u64,
        vault_balance_after_deposit: u64, protected_base_fees: u64,
    ) -> Result<(u64, u64)> {
        let leftover = base_reserve.safe_sub(vested)?.safe_sub(deposited)?;
        require!(vault_balance_after_deposit >= leftover.safe_add(protected_base_fees)?, PoolError::InvalidAccount);
        let burnable = self.get_burnable_amount_post_migration(leftover)?;
        if self.is_fixed_token_supply() && self.post_migration_token_supply == 0 {
            // A zero-floor policy must never silently credit excess inventory
            // to treasury. Reject a corrupt ledger beyond the issued supply.
            require!(burnable == leftover, PoolError::InvalidTokenSupply);
        }
        Ok((burnable, leftover.safe_sub(burnable)?))
    }

    pub fn is_fixed_token_supply(&self) -> bool {
        self.fixed_token_supply_flag == 1
    }

    pub fn split_partner_and_creator_fee(&self, fee: u64) -> Result<PartnerAndCreatorSplitFee> {
        // early return
        if self.creator_trading_fee_percentage == 0 {
            return Ok(PartnerAndCreatorSplitFee {
                partner_fee: fee,
                creator_fee: 0,
            });
        }
        let creator_fee = safe_mul_div_cast_u64(
            fee,
            self.creator_trading_fee_percentage.into(),
            100,
            Rounding::Down,
        )?;
        let partner_fee = fee.safe_sub(creator_fee)?;
        Ok(PartnerAndCreatorSplitFee {
            partner_fee,
            creator_fee,
        })
    }

    pub fn split_pool_creation_fee(&self) -> Result<(u64, u64)> {
        let protocol_fee = safe_mul_div_cast_u64(
            self.pool_creation_fee,
            PROTOCOL_POOL_CREATION_FEE_PERCENT.into(),
            100,
            Rounding::Down,
        )?;
        let partner_fee = self.pool_creation_fee.safe_sub(protocol_fee)?;
        Ok((protocol_fee, partner_fee))
    }
}

pub struct PartnerAndCreatorSplitFee {
    pub partner_fee: u64,
    pub creator_fee: u64,
}

#[cfg(test)]
mod launchpad_fee_tests {
    use super::*;

    #[test]
    fn launchpad_fee_split_has_no_protocol_or_referral_skim() {
        let fees = PoolFeesConfig::default();
        let mut config = PoolConfig::default();
        config.creator_trading_fee_percentage = 50;
        // Include dust, both selected tiers on a 1 SOL trade, and u64 limits.
        for amount in [0, 1, 2, 3, 10_000_000, 20_000_000, u64::MAX] {
            for has_referral in [false, true] {
                let (collected, protocol, referral) =
                    fees.split_fees(amount, has_referral).unwrap();
                assert_eq!((collected, protocol, referral), (amount, 0, 0));
                let split = config.split_partner_and_creator_fee(collected).unwrap();
                assert_eq!(split.creator_fee, amount / 2);
                assert_eq!(split.partner_fee, amount - amount / 2);
                assert_eq!(
                    split.creator_fee.checked_add(split.partner_fee),
                    Some(amount)
                );
            }
        }
    }
}
