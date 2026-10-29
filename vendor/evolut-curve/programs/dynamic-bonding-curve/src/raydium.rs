//! Pinned official Raydium ABI used by the atomic graduation adapter.
//! Layout provenance: raydium-io/raydium-cp-swap states/{config,pool}.rs and
//! raydium-io/raydium-cpi programs/locking-cpi/src/states.rs. These Borsh field
//! declarations mirror the captured deployed ABI; no unvalidated offset reads.
use crate::PoolError;
use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program::invoke_signed,
        pubkey,
    },
};

pub const EVOLUT_AUTHORITY: Pubkey = crate::treasury::ID;
/// Separate collection authority per source pool, never a shared WSOL bucket.
pub const CREATOR_AUTH_SEED: &[u8] = b"ray_creator_fees";
pub const MAINNET_CREATOR_CONFIG: Pubkey = pubkey!("9UQpPzgjju8DhHGcdnGDuXck4VGnHzGhRxT2NmEr6aS8");
pub const MAINNET_LP_CONFIG: Pubkey = pubkey!("2fGXL8uhqxJ4tpgtosHZXT4zcQap6j62z3bMDxdkMvy5");
#[cfg(not(feature = "mainnet"))]
pub const CPMM_ID: Pubkey = pubkey!("DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb");
#[cfg(feature = "mainnet")]
pub const CPMM_ID: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
#[cfg(not(feature = "mainnet"))]
pub const LOCK_ID: Pubkey = pubkey!("DRay25Usp3YJAi7beckgpGUC7mGJ2cR1AVPxhYfwVCUX");
#[cfg(feature = "mainnet")]
pub const LOCK_ID: Pubkey = pubkey!("LockrWmn6K5twhz3y9w1dQERbmgSaRkfnTeTKbpofwE");
#[cfg(not(feature = "mainnet"))]
pub const CREATE_FEE_RECEIVER: Pubkey = pubkey!("3oE58BKVt8KuYkGxx8zBojugnymWmBiyafWgMrnb6eYy");
#[cfg(feature = "mainnet")]
pub const CREATE_FEE_RECEIVER: Pubkey = pubkey!("DNXgeM9EiiaAbaWvwjHj9fQQLAX5ZsfHyvmYUNRAdNC8");
pub const METADATA_ID: Pubkey = pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
pub const POOL_SEED: &[u8] = b"ray_pool";
pub const NFT_SEED: &[u8] = b"ray_fee_nft";

#[derive(AnchorDeserialize)]
pub struct AmmConfig {
    pub bump: u8,
    pub disable_create_pool: bool,
    pub index: u16,
    pub trade_fee_rate: u64,
    pub protocol_fee_rate: u64,
    pub fund_fee_rate: u64,
    pub create_pool_fee: u64,
    pub protocol_owner: Pubkey,
    pub fund_owner: Pubkey,
    pub creator_fee_rate: u64,
    pub creator_fee_share_rate: u64,
    pub padding: [u64; 14],
}

#[derive(AnchorDeserialize)]
pub struct CpmmPool {
    pub amm_config: Pubkey,
    pub pool_creator: Pubkey,
    pub token_0_vault: Pubkey,
    pub token_1_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
    pub token_0_program: Pubkey,
    pub token_1_program: Pubkey,
    pub observation_key: Pubkey,
    pub auth_bump: u8,
    pub status: u8,
    pub lp_mint_decimals: u8,
    pub mint_0_decimals: u8,
    pub mint_1_decimals: u8,
    pub lp_supply: u64,
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,
    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,
    pub open_time: u64,
    pub recent_epoch: u64,
    pub creator_fee_on: u8,
    pub enable_creator_fee: bool,
    pub padding1: [u8; 6],
    pub creator_fees_token_0: u64,
    pub creator_fees_token_1: u64,
    pub padding: [u64; 28],
}

#[derive(AnchorDeserialize)]
pub struct LockedLiquidity {
    pub locked_lp_amount: u64,
    pub claimed_lp_amount: u64,
    pub unclaimed_lp_amount: u64,
    pub last_lp_amount: u64,
    pub last_k: u128,
    pub recent_epoch: u64,
    pub pool_id: Pubkey,
    pub fee_nft_mint: Pubkey,
    pub locked_owner: Pubkey,
    pub lp_mint: Pubkey,
    pub padding: [u64; 8],
}

fn decode<T: AnchorDeserialize>(
    account: &AccountInfo,
    owner: Pubkey,
    discriminator: [u8; 8],
    size: usize,
) -> Result<T> {
    require_keys_eq!(*account.owner, owner, PoolError::InvalidAccount);
    require!(
        !account.executable && account.data_len() == size,
        PoolError::InvalidAccount
    );
    let data = account.try_borrow_data()?;
    require!(data[..8] == discriminator, PoolError::InvalidAccount);
    T::try_from_slice(&data[8..]).map_err(|_| PoolError::InvalidAccount.into())
}
pub fn decode_pool(account: &AccountInfo) -> Result<CpmmPool> {
    decode(
        account,
        CPMM_ID,
        [247, 237, 227, 245, 215, 195, 222, 70],
        637,
    )
}
pub fn decode_lock(account: &AccountInfo) -> Result<LockedLiquidity> {
    decode(account, LOCK_ID, [25, 10, 238, 197, 207, 234, 73, 22], 256)
}
pub fn creator_fee_route(config: &Pubkey) -> bool {
    cfg!(feature = "mainnet") && *config == MAINNET_CREATOR_CONFIG
}

#[derive(AnchorDeserialize)]
struct Permission {
    authority: Pubkey,
    _padding: [u64; 30],
}

pub fn validate_permission(account: &AccountInfo, payer: &Pubkey) -> Result<()> {
    require_pda(account, &CPMM_ID, &[b"permission", payer.as_ref()])?;
    let permission: Permission = decode(account, CPMM_ID, [224, 83, 28, 79, 10, 253, 161, 28], 280)?;
    require_keys_eq!(permission.authority, *payer, PoolError::InvalidAccount);
    Ok(())
}
pub fn validate_fee_config(account: &AccountInfo) -> Result<AmmConfig> {
    let config: AmmConfig = decode(
        account,
        CPMM_ID,
        [218, 244, 33, 104, 203, 203, 43, 111],
        236,
    )?;
    let (expected, bump) =
        Pubkey::find_program_address(&[b"amm_config", &config.index.to_be_bytes()], &CPMM_ID);
    require_keys_eq!(account.key(), expected, PoolError::InvalidAccount);
    require!(
        config.bump == bump && !config.disable_create_pool,
        PoolError::InvalidInput
    );
    #[cfg(not(feature = "mainnet"))]
    require!(
        account.key() == pubkey!("EsTevfacYXpuho5VBuzBjDZi8dtWidGnXoSYAr8krTvz")
            && config.trade_fee_rate == 10_000,
        PoolError::InvalidInput
    );
    #[cfg(feature = "mainnet")]
    require!(
        (account.key() == MAINNET_CREATOR_CONFIG
            && config.trade_fee_rate == 300 && config.creator_fee_rate == 9_700)
            || (account.key() == MAINNET_LP_CONFIG
                && config.trade_fee_rate == 20_000),
        PoolError::InvalidInput
    );
    require!(
        config.protocol_fee_rate == 120_000
            && config.fund_fee_rate == 40_000
            && config.creator_fee_rate < 1_000_000,
        PoolError::InvalidInput
    );
    Ok(config)
}

/// Creation policy is separate from the external pool's trading fee. Existing
/// config/graduation validation above remains compatible with earlier pools.
/// New mainnet configurations match the curve fee to the selected AMM tier.
/// Previously created configurations retain their immutable curve fees.
/// Mainnet #13 uses 0.03% base + 0.97% creator; #2 uses 2% base, creator disabled.
pub fn validate_new_launch_fee(
    raydium_config_index: u16,
    raydium_trade_fee_rate: u64,
    curve_fee_numerator: u64,
) -> Result<()> {
    #[cfg(feature = "mainnet")]
    require!(
        (raydium_config_index == 2 && raydium_trade_fee_rate == 20_000
            && curve_fee_numerator == 20_000_000)
            || (raydium_config_index == 13 && raydium_trade_fee_rate == 300
                && curve_fee_numerator == 10_000_000),
        PoolError::InvalidInput
    );
    #[cfg(not(feature = "mainnet"))]
    {
        let _ = raydium_config_index;
        require!(
            raydium_trade_fee_rate.checked_mul(1000) == Some(curve_fee_numerator),
            PoolError::InvalidInput
        );
    }
    Ok(())
}
pub fn require_pda(account: &AccountInfo, program: &Pubkey, seeds: &[&[u8]]) -> Result<()> {
    require_keys_eq!(
        account.key(),
        Pubkey::find_program_address(seeds, program).0,
        PoolError::InvalidAccount
    );
    Ok(())
}
pub fn require_ata(account: &AccountInfo, owner: &Pubkey, mint: &Pubkey) -> Result<()> {
    require_keys_eq!(
        account.key(),
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            owner,
            mint,
            &anchor_spl::token::ID
        ),
        PoolError::InvalidAccount
    );
    Ok(())
}
/// All destination accounts must be fresh system accounts. Prefunding is allowed and
/// reduces reimbursement by the exact rent shortfall; initialized account substitution is not.
pub fn creation_shortfall(account: &AccountInfo, size: usize, rent: &Rent) -> Result<u64> {
    require_keys_eq!(
        *account.owner,
        anchor_lang::system_program::ID,
        PoolError::InvalidAccount
    );
    require!(
        !account.executable && account.data_is_empty(),
        PoolError::InvalidAccount
    );
    Ok(rent
        .minimum_balance(size)
        .saturating_sub(account.lamports()))
}

/// Explicit CPI privileges from the official Initialize/LockCpLiquidity interfaces.
pub fn invoke_abi<'info>(
    program: &AccountInfo<'info>,
    accounts: &[(&AccountInfo<'info>, bool, bool)],
    data: Vec<u8>,
    seeds: &[&[&[u8]]],
) -> Result<()> {
    let metas = accounts
        .iter()
        .map(|(account, writable, signer)| {
            if *writable {
                AccountMeta::new(account.key(), *signer)
            } else {
                AccountMeta::new_readonly(account.key(), *signer)
            }
        })
        .collect();
    let mut infos = Vec::with_capacity(accounts.len() + 1);
    infos.extend(accounts.iter().map(|(account, _, _)| (*account).clone()));
    infos.push(program.clone());
    invoke_signed(
        &Instruction {
            program_id: program.key(),
            accounts: metas,
            data,
        },
        &infos,
        seeds,
    )?;
    Ok(())
}
pub fn initialize_data(amount_0: u64, amount_1: u64) -> Vec<u8> {
    let mut data = vec![175, 175, 109, 31, 13, 152, 155, 237];
    for amount in [amount_0, amount_1, 0] {
        data.extend_from_slice(&amount.to_le_bytes());
    }
    data
}
pub fn initialize_permission_data(amount_0: u64, amount_1: u64) -> Vec<u8> {
    let mut data = vec![63, 55, 254, 65, 49, 178, 89, 121];
    for amount in [amount_0, amount_1, 0] {
        data.extend_from_slice(&amount.to_le_bytes());
    }
    data.push(0); // BothToken: creator fee on input for both buy and sell.
    data
}
pub fn lock_data(amount: u64) -> Vec<u8> {
    let mut data = vec![216, 157, 29, 78, 38, 51, 31, 26];
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(0); // No artwork metadata: fee rights and permanent lock are unchanged.
    data
}

#[cfg(test)]
mod fee_policy_tests {
    use super::*;

    #[test]
    fn creator_share_decodes_without_changing_external_account_size() {
        // AmmConfig is 228 Borsh bytes plus its eight-byte discriminator.
        let mut bytes = vec![0u8; 228];
        bytes[100..108].copy_from_slice(&10_000u64.to_le_bytes());
        bytes[108..116].copy_from_slice(&50_000u64.to_le_bytes());
        bytes[220..228].copy_from_slice(&77u64.to_le_bytes());
        let config = AmmConfig::try_from_slice(&bytes).unwrap();
        assert_eq!(config.creator_fee_rate, 10_000);
        assert_eq!(config.creator_fee_share_rate, 50_000);
        assert_eq!(config.padding[13], 77);
        assert!(AmmConfig::try_from_slice(&bytes[..227]).is_err());
    }

    #[cfg(feature = "mainnet")]
    #[test]
    fn new_mainnet_configs_match_curve_and_graduated_fee_tiers() {
        assert!(validate_new_launch_fee(2, 20_000, 20_000_000).is_ok());
        assert!(validate_new_launch_fee(13, 300, 10_000_000).is_ok());
        assert!(validate_new_launch_fee(2, 20_000, 10_000_000).is_err());
        assert!(validate_new_launch_fee(13, 300, 20_000_000).is_err());
        assert!(validate_new_launch_fee(13, 10_000, 10_000_000).is_err());
        assert!(validate_new_launch_fee(13, 300, 300_000).is_err());
        for curve_fee in [0, 2_500_000, 12_500_000, 30_000_000, u64::MAX] {
            assert!(validate_new_launch_fee(2, 20_000, curve_fee).is_err());
            assert!(validate_new_launch_fee(13, 300, curve_fee).is_err());
        }
        assert!(validate_new_launch_fee(2, 10_000, 10_000_000).is_err());
    }

    #[cfg(feature = "mainnet")]
    #[test]
    fn unsupported_creator_route_and_superseded_launch_tier_stay_closed() {
        // Ordinary Initialize on #11 would charge only its base fee, not the
        // promised creator fee; accepting it now would create the wrong pool.
        assert!(validate_new_launch_fee(11, 2_500, 10_000_000).is_err());
        assert!(validate_new_launch_fee(1, 10_000, 10_000_000).is_err());
        assert!(validate_new_launch_fee(99, 20_000, 10_000_000).is_err());
    }

    #[cfg(not(feature = "mainnet"))]
    #[test]
    fn devnet_fee_policy_is_unchanged_and_overflow_is_rejected() {
        assert!(validate_new_launch_fee(0, 10_000, 10_000_000).is_ok());
        assert!(validate_new_launch_fee(0, 10_000, 20_000_000).is_err());
        assert!(validate_new_launch_fee(0, u64::MAX, 0).is_err());
    }
}
