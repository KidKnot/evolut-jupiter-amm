//! Event module includes information about events of the program
use anchor_lang::prelude::*;

use crate::{
    state::{SwapResult, SwapResult2},
    ConfigParameters, SwapParameters, SwapParameters2,
};

/// Create partner metadata
#[event]
pub struct EvtPartnerMetadata {
    pub partner_metadata: Pubkey,
    pub fee_claimer: Pubkey,
}

/// Create virtual pool metadata
#[event]
pub struct EvtVirtualPoolMetadata {
    pub virtual_pool_metadata: Pubkey,
    pub virtual_pool: Pubkey,
}

#[event]
pub struct EvtCreateConfigV2 {
    pub config: Pubkey,
    pub quote_mint: Pubkey,
    pub fee_claimer: Pubkey,
    pub leftover_receiver: Pubkey,
    pub config_parameters: ConfigParameters,
}

/// Create claim fee operator
#[event]
pub struct EvtCreateClaimFeeOperator {
    pub operator: Pubkey,
}

/// Close claim fee operator
#[event]
pub struct EvtCloseClaimFeeOperator {
    pub claim_fee_operator: Pubkey,
    pub operator: Pubkey,
}

#[event]
pub struct EvtInitializePool {
    pub pool: Pubkey,
    pub config: Pubkey,
    pub creator: Pubkey,
    pub base_mint: Pubkey,
    pub pool_type: u8,
    pub activation_point: u64,
}

#[event]
pub struct EvtSwap {
    pub pool: Pubkey,
    pub config: Pubkey,
    pub trade_direction: u8,
    pub has_referral: bool,
    pub params: SwapParameters,
    pub swap_result: SwapResult,
    pub amount_in: u64,
    pub current_timestamp: u64,
}

#[event]
pub struct EvtSwap2 {
    pub pool: Pubkey,
    pub config: Pubkey,
    pub trade_direction: u8,
    pub has_referral: bool,
    pub swap_parameters: SwapParameters2,
    pub swap_result: SwapResult2,
    pub quote_reserve_amount: u64,
    pub migration_threshold: u64,
    pub current_timestamp: u64,
}

#[event]
pub struct EvtCurveComplete {
    pub pool: Pubkey,
    pub config: Pubkey,
    pub base_reserve: u64,
    pub quote_reserve: u64,
}

/// Emitted by a self-CPI only after the atomic graduation postconditions pass.
#[event]
pub struct PoolGraduated {
    pub virtual_pool: Pubkey,
    pub config: Pubkey,
    pub raydium_amm_config: Pubkey,
    pub raydium_pool: Pubkey,
    pub lp_mint: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub token_0_vault: Pubkey,
    pub token_1_vault: Pubkey,
    pub creator_fee_nft: Pubkey,
    pub partner_fee_nft: Pubkey,
    pub creator_lock: Pubkey,
    pub partner_lock: Pubkey,
    pub initial_creator: Pubkey,
    pub initial_partner: Pubkey,
    pub nonce: [u8; 32],
    pub creator_lp_amount: u64,
    pub partner_lp_amount: u64,
    pub setup_lamports: u64,
    pub base_deposited: u64,
    pub quote_deposited: u64,
    pub slot: u64,
}

#[event]
pub struct EvtClaimProtocolFee {
    pub pool: Pubkey,
    pub token_base_amount: u64,
    pub token_quote_amount: u64,
}

#[event]
pub struct EvtClaimTradingFee {
    pub pool: Pubkey,
    pub token_base_amount: u64,
    pub token_quote_amount: u64,
}

#[event]
pub struct EvtClaimCreatorTradingFee {
    pub pool: Pubkey,
    pub token_base_amount: u64,
    pub token_quote_amount: u64,
}

#[event]
pub struct EvtProtocolWithdrawSurplus {
    pub pool: Pubkey,
    pub surplus_amount: u64,
}

#[event]
pub struct EvtPartnerWithdrawSurplus {
    pub pool: Pubkey,
    pub surplus_amount: u64,
}

#[event]
pub struct EvtCreatorWithdrawSurplus {
    pub pool: Pubkey,
    pub surplus_amount: u64,
}

#[event]
pub struct EvtWithdrawLeftover {
    pub pool: Pubkey,
    pub leftover_receiver: Pubkey,
    pub leftover_amount: u64,
}

#[event]
pub struct EvtUpdatePoolCreator {
    pub pool: Pubkey,
    pub creator: Pubkey,
    pub new_creator: Pubkey,
}

#[event]
pub struct EvtClaimPoolCreationFee {
    pub pool: Pubkey,
    pub receiver: Pubkey,
    pub creation_fee: u64,
}

#[event]
pub struct EvtPartnerClaimPoolCreationFee {
    pub pool: Pubkey,
    pub partner: Pubkey,
    pub creation_fee: u64,
}
