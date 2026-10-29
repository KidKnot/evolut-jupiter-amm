use crate::{
    activation_handler::ActivationType,
    constants::{
        fee::{MAX_POOL_CREATION_FEE, MIN_POOL_CREATION_FEE},
        MAX_CURVE_POINT, MAX_SQRT_PRICE, MIN_SQRT_PRICE,
    },
    params::{
        fee_parameters::PoolFeeParameters,
        liquidity_distribution::{
            get_base_token_for_swap, get_migration_base_token, get_migration_threshold_price,
            LiquidityDistributionParameters,
        },
    },
    raydium::{validate_fee_config, validate_new_launch_fee, EVOLUT_AUTHORITY},
    safe_math::SafeMath,
    state::{CollectFeeMode, LockedVestingConfig, PoolConfig, TokenAuthorityOption},
    EvtCreateConfigV2, PoolError,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use locker::types::CreateVestingEscrowParameters;

/// Fresh Raydium-only immutable launch policy. No migration percentage fee or LP unlock path.
#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct ConfigParameters {
    pub pool_fees: PoolFeeParameters,
    pub collect_fee_mode: u8,
    pub activation_type: u8,
    pub token_decimal: u8,
    pub migration_quote_threshold: u64,
    pub sqrt_start_price: u128,
    pub locked_vesting: LockedVestingParams,
    pub token_supply: Option<TokenSupplyParams>,
    pub token_update_authority: u8,
    pub pool_creation_fee: u64,
    /// Disclosed maximum actual account rent + official pool creation cost.
    pub max_setup_lamports: u64,
    pub curve: Vec<LiquidityDistributionParameters>,
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct TokenSupplyParams {
    /// pre migration token supply
    pub pre_migration_token_supply: u64,
    /// Legacy minimum supply after graduation. New mainnet policies require
    /// zero: no supply floor protects unused inventory from burning. This does
    /// NOT mean burn holders' tokens or set actual mint supply to zero.
    pub post_migration_token_supply: u64,
}

impl TokenSupplyParams {
    pub fn validate_supply_requirements(&self, minimum_with_buffer: u64, minimum_without_buffer: u64) -> Result<()> {
        let permitted_floor = if cfg!(feature = "mainnet") {
            self.post_migration_token_supply == 0
        } else {
            minimum_without_buffer <= self.post_migration_token_supply
        };
        require!(
            permitted_floor
                && self.post_migration_token_supply <= self.pre_migration_token_supply
                && minimum_without_buffer <= minimum_with_buffer
                && minimum_with_buffer <= self.pre_migration_token_supply,
            PoolError::InvalidTokenSupply
        );
        Ok(())
    }
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct LockedVestingParams {
    pub amount_per_period: u64,
    pub cliff_duration_from_migration_time: u64,
    pub frequency: u64,
    pub number_of_period: u64,
    pub cliff_unlock_amount: u64,
}

impl LockedVestingParams {
    pub fn to_locked_vesting_config(&self) -> LockedVestingConfig {
        LockedVestingConfig {
            amount_per_period: self.amount_per_period,
            cliff_duration_from_migration_time: self.cliff_duration_from_migration_time,
            frequency: self.frequency,
            number_of_period: self.number_of_period,
            cliff_unlock_amount: self.cliff_unlock_amount,
            ..Default::default()
        }
    }

    pub fn to_create_vesting_escrow_params(
        &self,
        finish_curve_timestamp: u64,
    ) -> Result<CreateVestingEscrowParameters> {
        let cliff_time =
            finish_curve_timestamp.safe_add(self.cliff_duration_from_migration_time)?;
        Ok(CreateVestingEscrowParameters {
            vesting_start_time: finish_curve_timestamp,
            cliff_time,
            frequency: self.frequency,
            cliff_unlock_amount: self.cliff_unlock_amount,
            amount_per_period: self.amount_per_period,
            number_of_period: self.number_of_period,
            update_recipient_mode: 2, // only recipient
            cancel_mode: 1,           // only creator
        })
    }

    pub fn get_total_amount(&self) -> Result<u64> {
        let total_amount = self
            .cliff_unlock_amount
            .safe_add(self.amount_per_period.safe_mul(self.number_of_period)?)?;
        Ok(total_amount)
    }

    pub fn has_vesting(&self) -> bool {
        *self != LockedVestingParams::default()
    }
    pub fn validate(&self) -> Result<()> {
        if self.has_vesting() {
            let total_amount = self.get_total_amount()?;
            require!(
                self.frequency != 0 && total_amount != 0,
                PoolError::InvalidVestingParameters
            );
        }
        Ok(())
    }
}

impl ConfigParameters {
    pub fn validate(&self) -> Result<()> {
        let activation = ActivationType::try_from(self.activation_type)
            .map_err(|_| PoolError::TypeCastFailed)?;
        self.pool_fees.validate(self.collect_fee_mode, activation)?;
        require!(
            CollectFeeMode::try_from(self.collect_fee_mode).is_ok(),
            PoolError::InvalidCollectFeeMode
        );
        require!(
            TokenAuthorityOption::try_from(self.token_update_authority).is_ok(),
            PoolError::InvalidTokenAuthorityOption
        );
        // The advertised tier is exact throughout the curve, not a scheduler or dynamic fee.
        let base = self.pool_fees.base_fee;
        require!(
            base.first_factor == 0
                && base.second_factor == 0
                && base.third_factor == 0
                && base.base_fee_mode == 0
                && self.pool_fees.dynamic_fee.is_none(),
            PoolError::InvalidInput
        );
        require!(
            self.token_decimal >= 6 && self.token_decimal <= 9,
            PoolError::InvalidTokenDecimals
        );
        require!(
            self.max_setup_lamports > 0 && self.max_setup_lamports < self.migration_quote_threshold,
            PoolError::InvalidQuoteThreshold
        );
        self.locked_vesting.validate()?;
        #[cfg(feature = "mainnet")]
        {
            let graduation_fee = PoolConfig::mainnet_graduation_fee(self.migration_quote_threshold)?;
            require!(self.max_setup_lamports.safe_add(graduation_fee)? < self.migration_quote_threshold,
                PoolError::InvalidQuoteThreshold);
            // Owner-approved mainnet defaults. These constrain NEW policies;
            // they do not reinterpret existing devnet pools or mint authority.
            require!(self.token_decimal == 6, PoolError::InvalidTokenDecimals);
            require!(
                matches!(self.token_update_authority, 0 | 2),
                PoolError::InvalidTokenAuthorityOption
            );
            require!(self.locked_vesting == LockedVestingParams::default(), PoolError::InvalidInput);
            require!(
                self.token_supply.map(|s| s.pre_migration_token_supply == 1_000_000_000_000_000
                    && s.post_migration_token_supply == 0)
                    == Some(true),
                PoolError::InvalidTokenSupply
            );
            // Platform launch revenue is transferred atomically during mainnet
            // initialization; do not also accrue this legacy pool-account fee.
            require!(self.pool_creation_fee == 0, PoolError::InvalidPoolCreationFee);
        }
        if self.pool_creation_fee > 0 {
            require!(
                self.pool_creation_fee >= MIN_POOL_CREATION_FEE
                    && self.pool_creation_fee <= MAX_POOL_CREATION_FEE,
                PoolError::InvalidPoolCreationFee
            );
        }
        require!(
            self.sqrt_start_price >= MIN_SQRT_PRICE && self.sqrt_start_price < MAX_SQRT_PRICE,
            PoolError::InvalidCurve
        );
        require!(
            !self.curve.is_empty() && self.curve.len() <= MAX_CURVE_POINT,
            PoolError::InvalidCurve
        );
        let mut previous = self.sqrt_start_price;
        for point in &self.curve {
            require!(
                point.sqrt_price > previous
                    && point.sqrt_price <= MAX_SQRT_PRICE
                    && point.liquidity > 0,
                PoolError::InvalidCurve
            );
            previous = point.sqrt_price;
        }
        Ok(())
    }
}

#[event_cpi]
#[derive(Accounts)]
pub struct CreateConfigCtx<'info> {
    #[account(init, signer, payer = payer, space = 8 + PoolConfig::INIT_SPACE)]
    pub config: AccountLoader<'info, PoolConfig>,
    /// The canonical Evolut signer also owns the immutable partner fee rights.
    #[account(address = EVOLUT_AUTHORITY)]
    pub fee_claimer: Signer<'info>,
    /// CHECK: leftovers belong to the treasury, never an operational signer.
    #[account(address = crate::treasury::ID)]
    pub leftover_receiver: UncheckedAccount<'info>,
    #[account(address = anchor_spl::token::spl_token::native_mint::ID)]
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,
    /// CHECK: official owner, discriminator, PDA, enabled state and exact fee policy checked below.
    pub raydium_amm_config: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_create_config(
    ctx: Context<CreateConfigCtx>,
    config_parameters: ConfigParameters,
) -> Result<()> {
    config_parameters.validate()?;
    require_keys_eq!(
        *ctx.accounts.quote_mint.to_account_info().owner,
        anchor_spl::token::ID,
        PoolError::InvalidQuoteMint
    );
    let ray = validate_fee_config(&ctx.accounts.raydium_amm_config)?;
    validate_new_launch_fee(
        ray.index,
        ray.trade_fee_rate,
        config_parameters.pool_fees.base_fee.cliff_fee_numerator,
    )?;
    require!(
        ray.create_pool_fee < config_parameters.max_setup_lamports,
        PoolError::InvalidInput
    );
    let ConfigParameters {
        pool_fees,
        collect_fee_mode,
        activation_type,
        token_decimal,
        migration_quote_threshold,
        sqrt_start_price,
        locked_vesting,
        token_supply,
        token_update_authority,
        pool_creation_fee,
        max_setup_lamports,
        curve,
    } = config_parameters.clone();
    let sqrt_migration_price =
        get_migration_threshold_price(migration_quote_threshold, sqrt_start_price, &curve)?;
    require!(
        sqrt_migration_price < MAX_SQRT_PRICE,
        PoolError::InvalidCurve
    );
    let swap_base_amount: u64 =
        get_base_token_for_swap(sqrt_start_price, sqrt_migration_price, &curve)?
            .try_into()
            .map_err(|_| PoolError::TypeCastFailed)?;
    // Reserve the worst-case base at full T. Setup reimbursement only reduces the actual deposit.
    let migration_base_amount =
        get_migration_base_token(migration_quote_threshold, sqrt_migration_price)?;
    require!(
        migration_base_amount > 0 && swap_base_amount > 0,
        PoolError::InvalidCurve
    );
    let (fixed_token_supply_flag, pre_migration_token_supply, post_migration_token_supply) =
        if let Some(TokenSupplyParams {
            pre_migration_token_supply,
            post_migration_token_supply,
        }) = token_supply
        {
            let swap_base_amount_buffer = PoolConfig::get_swap_amount_with_buffer(
                swap_base_amount,
                sqrt_start_price,
                &curve,
            )?;

            let minimum_base_supply_with_buffer = PoolConfig::get_total_token_supply(
                swap_base_amount_buffer,
                migration_base_amount,
                &locked_vesting,
            )?;

            let minimum_base_supply_without_buffer = PoolConfig::get_total_token_supply(
                swap_base_amount,
                migration_base_amount,
                &locked_vesting,
            )?;

            require!(
                ctx.accounts.leftover_receiver.key() != Pubkey::default(),
                PoolError::InvalidLeftoverAddress
            );
            TokenSupplyParams { pre_migration_token_supply, post_migration_token_supply }
                .validate_supply_requirements(minimum_base_supply_with_buffer, minimum_base_supply_without_buffer)?;
            (1, pre_migration_token_supply, post_migration_token_supply)
        } else {
            (0, 0, 0)
        };

    let mut config = ctx.accounts.config.load_init()?;
    config.quote_mint = ctx.accounts.quote_mint.key();
    config.fee_claimer = ctx.accounts.fee_claimer.key();
    config.leftover_receiver = ctx.accounts.leftover_receiver.key();
    config.raydium_amm_config = ctx.accounts.raydium_amm_config.key();
    config.pool_fees = pool_fees.to_pool_fees_config();
    config.locked_vesting_config = locked_vesting.to_locked_vesting_config();
    config.migration_sqrt_price = sqrt_migration_price;
    config.sqrt_start_price = sqrt_start_price;
    config.swap_base_amount = swap_base_amount;
    config.migration_quote_threshold = migration_quote_threshold;
    config.migration_base_threshold = migration_base_amount;
    config.pre_migration_token_supply = pre_migration_token_supply;
    config.post_migration_token_supply = post_migration_token_supply;
    config.pool_creation_fee = pool_creation_fee;
    config.max_setup_lamports = max_setup_lamports;
    config.raydium_trade_fee_rate = ray.trade_fee_rate;
    config.raydium_protocol_fee_rate = ray.protocol_fee_rate;
    config.raydium_fund_fee_rate = ray.fund_fee_rate;
    config.raydium_create_pool_fee = ray.create_pool_fee;
    config.collect_fee_mode = collect_fee_mode;
    config.activation_type = activation_type;
    config.token_decimal = token_decimal;
    config.version = 1;
    #[cfg(feature = "mainnet")]
    {
        config.version = 2;
        config._padding[..8].copy_from_slice(
            &PoolConfig::mainnet_graduation_fee(migration_quote_threshold)?.to_le_bytes());
    }
    config.token_type = 0;
    config.quote_token_flag = 0;
    config.fixed_token_supply_flag = fixed_token_supply_flag;
    config.creator_trading_fee_percentage = 50;
    config.token_update_authority = token_update_authority;
    for (i, point) in curve.iter().enumerate() {
        config.curve[i] = point.to_liquidity_distribution_config();
    }
    drop(config);
    emit_cpi!(EvtCreateConfigV2 {
        config: ctx.accounts.config.key(),
        quote_mint: ctx.accounts.quote_mint.key(),
        fee_claimer: ctx.accounts.fee_claimer.key(),
        leftover_receiver: ctx.accounts.leftover_receiver.key(),
        config_parameters,
    });
    Ok(())
}
