//! Mainnet platform launch revenue is transferred by initialization itself.
//! The amount and recipient are program policy, never client-provided proof.
//! All allocations and this transfer roll back if any part of the launch fails.
use anchor_lang::prelude::*;

use crate::{state::TokenAuthorityOption, token::transfer_lamports_from_user, PoolError};

pub const MAINNET_EVOLVING_LAUNCH_FEE_LAMPORTS: u64 = 15_000_000;
pub const MAINNET_CLASSIC_LAUNCH_FEE_LAMPORTS: u64 = 15_000_000;

/// The owner-approved fee applies to both mainnet products and old configuration
/// accounts when used for a NEW pool. Config selection cannot waive the fee.
pub fn launch_fee_lamports(authority: TokenAuthorityOption) -> u64 {
    if !cfg!(feature = "mainnet") {
        return 0;
    }
    match authority {
        TokenAuthorityOption::PartnerUpdateAuthority
        | TokenAuthorityOption::PartnerUpdateAndMintAuthority => {
            MAINNET_EVOLVING_LAUNCH_FEE_LAMPORTS
        }
        _ => MAINNET_CLASSIC_LAUNCH_FEE_LAMPORTS,
    }
}

/// Append the canonical treasury as a writable remaining account. Keeping it
/// outside the fixed account list preserves the existing instruction encoding,
/// pool/config layout and trading/claim ABI. Omitting the account never waives a
/// nonzero fee, including when the treasury authorizes a legacy Evolving launch.
pub fn charge_launch_fee<'info>(
    authority: TokenAuthorityOption,
    payer: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let fee = launch_fee_lamports(authority);
    if fee == 0 {
        return Ok(());
    }
    let recipient = remaining_accounts
        .iter()
        .find(|account| *account.key == crate::admin::treasury::ID)
        .ok_or(PoolError::InvalidLaunchFeeRecipient)?;
    require!(
        recipient.is_writable && !recipient.executable,
        PoolError::InvalidLaunchFeeRecipient
    );
    require_keys_eq!(
        *recipient.owner,
        anchor_lang::system_program::ID,
        PoolError::InvalidLaunchFeeRecipient
    );
    transfer_lamports_from_user(payer, recipient.clone(), system_program, fee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mainnet_authority_mode_charges_the_same_approved_platform_fee() {
        for authority in [
            TokenAuthorityOption::PartnerUpdateAuthority,
            TokenAuthorityOption::PartnerUpdateAndMintAuthority,
        ] {
            assert_eq!(
                launch_fee_lamports(authority),
                if cfg!(feature = "mainnet") { 15_000_000 } else { 0 }
            );
        }
        for authority in [
            TokenAuthorityOption::CreatorUpdateAuthority,
            TokenAuthorityOption::CreatorUpdateAndMintAuthority,
            TokenAuthorityOption::Immutable,
        ] {
            assert_eq!(
                launch_fee_lamports(authority),
                if cfg!(feature = "mainnet") { 15_000_000 } else { 0 }
            );
        }
    }
}
