#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

#[macro_use]
pub mod macros;

pub mod const_pda;
pub mod instructions;
pub use instructions::*;
pub mod constants;
pub mod error;
pub mod state;
pub use error::*;
pub mod event;
pub use event::*;
pub mod utils;
pub use utils::*;
pub mod math;
pub use math::*;
pub mod access_control;
pub use access_control::*;
pub mod base_fee;
pub mod curve;

pub mod params;
pub mod raydium;

#[cfg(not(feature = "mainnet"))]
declare_id!("FHrKE2Pibu6Bd2Y9pHyjkHkEWeftTcy1DhN63dDwQa97");
#[cfg(feature = "mainnet")]
declare_id!("EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ");

#[program]
pub mod dynamic_bonding_curve {
    use super::*;

    pub fn initialize_operational_authority(ctx: Context<InitializeOperationalAuthorityCtx>, operator: Pubkey) -> Result<()> {
        instructions::handle_initialize_operational_authority(ctx, operator)
    }

    pub fn set_operational_authority(ctx: Context<SetOperationalAuthorityCtx>, operator: Pubkey) -> Result<()> {
        instructions::handle_set_operational_authority(ctx, operator)
    }

    pub fn update_operational_metadata(ctx: Context<UpdateOperationalMetadataCtx>, name: String) -> Result<()> {
        instructions::handle_update_operational_metadata(ctx, name)
    }

    pub fn quote_paid_launch_buy(ctx: Context<QuotePaidLaunchBuyCtx>, amount: u64) -> Result<()> {
        instructions::handle_quote_paid_launch_buy(ctx, amount)
    }

    #[access_control(is_admin(ctx.accounts.signer.key))]
    pub fn create_claim_protocol_fee_operator(
        ctx: Context<CreateClaimProtocolFeeOperatorCtx>,
    ) -> Result<()> {
        instructions::handle_create_claim_protocol_fee_operator(ctx)
    }

    #[access_control(is_admin(ctx.accounts.signer.key))]
    pub fn close_claim_protocol_fee_operator(
        ctx: Context<CloseClaimProtocolFeeOperatorCtx>,
    ) -> Result<()> {
        instructions::handle_close_claim_protocol_fee_operator(ctx)
    }

    #[access_control(is_claim_fee_operator(&ctx.accounts.claim_fee_operator, ctx.accounts.signer.key))]
    pub fn claim_protocol_fee(ctx: Context<ClaimProtocolFeesCtx>) -> Result<()> {
        instructions::handle_claim_protocol_fee(ctx)
    }

    #[access_control(is_claim_fee_operator(&ctx.accounts.claim_fee_operator, ctx.accounts.signer.key))]
    pub fn protocol_withdraw_surplus(ctx: Context<ProtocolWithdrawSurplusCtx>) -> Result<()> {
        instructions::handle_protocol_withdraw_surplus(ctx)
    }

    #[access_control(is_claim_fee_operator(&ctx.accounts.claim_fee_operator, ctx.accounts.signer.key))]
    pub fn claim_protocol_pool_creation_fee(
        ctx: Context<ClaimProtocolPoolCreationFeeCtx>,
    ) -> Result<()> {
        instructions::handle_claim_protocol_pool_creation_fee(ctx)
    }

    /// PARTNER FUNCTIONS ///
    pub fn create_partner_metadata(
        ctx: Context<CreatePartnerMetadataCtx>,
        metadata: CreatePartnerMetadataParameters,
    ) -> Result<()> {
        instructions::handle_create_partner_metadata(ctx, metadata)
    }

    pub fn create_config(
        ctx: Context<CreateConfigCtx>,
        config_parameters: ConfigParameters,
    ) -> Result<()> {
        instructions::handle_create_config(ctx, config_parameters)
    }

    #[access_control(is_partner_fee_claimer(&ctx.accounts.config, ctx.accounts.fee_claimer.key))]
    pub fn claim_trading_fee(
        ctx: Context<ClaimTradingFeesCtx>,
        max_amount_a: u64,
        max_amount_b: u64,
    ) -> Result<()> {
        instructions::handle_claim_trading_fee(ctx, max_amount_a, max_amount_b)
    }

    #[access_control(is_partner_fee_claimer(&ctx.accounts.config, ctx.accounts.fee_claimer.key))]
    pub fn claim_partner_pool_creation_fee(
        ctx: Context<ClaimPartnerPoolCreationFeeCtx>,
    ) -> Result<()> {
        instructions::handle_claim_partner_pool_creation_fee(ctx)
    }

    // withdraw surplus on quote token
    #[access_control(is_partner_fee_claimer(&ctx.accounts.config, ctx.accounts.fee_claimer.key))]
    pub fn partner_withdraw_surplus(ctx: Context<PartnerWithdrawSurplusCtx>) -> Result<()> {
        instructions::handle_partner_withdraw_surplus(ctx)
    }

    /// POOL CREATOR FUNCTIONS ////
    pub fn initialize_virtual_pool_with_spl_token<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, InitializeVirtualPoolWithSplTokenCtx<'info>>,
        params: InitializePoolParameters,
    ) -> Result<()> {
        instructions::handle_initialize_virtual_pool_with_spl_token(ctx, params)
    }

    #[access_control(is_pool_creator(&ctx.accounts.virtual_pool, ctx.accounts.creator.key))]
    pub fn create_virtual_pool_metadata(
        ctx: Context<CreateVirtualPoolMetadataCtx>,
        metadata: CreateVirtualPoolMetadataParameters,
    ) -> Result<()> {
        instructions::handle_create_virtual_pool_metadata(ctx, metadata)
    }

    #[access_control(is_pool_creator(&ctx.accounts.pool, ctx.accounts.creator.key))]
    pub fn claim_creator_trading_fee(
        ctx: Context<ClaimCreatorTradingFeesCtx>,
        max_base_amount: u64,
        max_quote_amount: u64,
    ) -> Result<()> {
        instructions::handle_claim_creator_trading_fee(ctx, max_base_amount, max_quote_amount)
    }

    // withdraw surplus on quote token
    #[access_control(is_pool_creator(&ctx.accounts.virtual_pool, ctx.accounts.creator.key))]
    pub fn creator_withdraw_surplus(ctx: Context<CreatorWithdrawSurplusCtx>) -> Result<()> {
        instructions::handle_creator_withdraw_surplus(ctx)
    }

    #[access_control(is_pool_creator(&ctx.accounts.virtual_pool, ctx.accounts.creator.key))]
    pub fn transfer_pool_creator<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, TransferPoolCreatorCtx>,
    ) -> Result<()> {
        instructions::handle_transfer_pool_creator(ctx)
    }

    /// TRADING BOTS FUNCTIONS ////
    pub fn swap(ctx: Context<SwapCtx>, params: SwapParameters) -> Result<()> {
        instructions::handle_swap_wrapper(
            ctx,
            SwapParameters2 {
                amount_0: params.amount_in,
                amount_1: params.minimum_amount_out,
                swap_mode: SwapMode::ExactIn.into(),
                ..Default::default()
            },
        )
    }

    pub fn swap2(ctx: Context<SwapCtx>, params: SwapParameters2) -> Result<()> {
        instructions::handle_swap_wrapper(ctx, params)
    }

    /// PERMISSIONLESS FUNCTIONS ///
    /// create locker
    pub fn create_locker(ctx: Context<CreateLockerCtx>) -> Result<()> {
        instructions::handle_create_locker(ctx)
    }

    // withdraw leftover on base token, can only call after pool is initialized
    pub fn withdraw_leftover(ctx: Context<WithdrawLeftoverCtx>) -> Result<()> {
        instructions::handle_withdraw_leftover(ctx)
    }

    /// Atomically initializes Raydium CPMM and permanently locks all issued LP.
    pub fn graduate<'info>(ctx: Context<'_, '_, '_, 'info, GraduateCtx<'info>>, nonce: [u8; 32]) -> Result<()> {
        instructions::handle_graduate(ctx, nonce)
    }

    /// Collect the net Raydium creator bucket and claim one FeeKey's 50% share.
    /// LP-fee claims remain independent calls to the official Raydium locker.
    pub fn claim_raydium_creator_fees(ctx: Context<ClaimRaydiumCreatorFeesCtx>, side: u8) -> Result<()> {
        instructions::handle_claim_raydium_creator_fees(ctx, side)
    }
}
