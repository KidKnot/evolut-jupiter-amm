use crate::{state::{TokenAuthorityOption, OperationalAuthority}, PoolError};
use anchor_lang::prelude::*;

/// Appending the canonical authority account opts a launch into PDA-managed
/// metadata. Original treasury-signed launches remain compatible; no existing
/// metadata or config account is rewritten and no treasury powers are delegated.
pub fn operational_launch_authority(
    authority: TokenAuthorityOption, partner: Pubkey, accounts: &[AccountInfo],
) -> Result<Option<(Pubkey, Pubkey)>> {
    if !matches!(authority, TokenAuthorityOption::PartnerUpdateAuthority
        | TokenAuthorityOption::PartnerUpdateAndMintAuthority) { return Ok(None); }
    let address = Pubkey::find_program_address(&[OperationalAuthority::SEED], &crate::ID).0;
    let Some(account) = accounts.iter().find(|a| *a.key == address) else { return Ok(None); };
    require_keys_eq!(partner, crate::admin::treasury::ID, PoolError::Unauthorized);
    require_keys_eq!(*account.owner, crate::ID, PoolError::Unauthorized);
    let data = account.try_borrow_data()?;
    let state = OperationalAuthority::try_deserialize(&mut &data[..])?;
    require!(state.operator != Pubkey::default(), PoolError::Unauthorized);
    Ok(Some((state.operator, address)))
}

/// Classic creator/immutable metadata is permissionless. A partner-managed
/// identity needs that partner's signature on this exact launch transaction.
/// For Evolut configs the partner is the platform treasury. A caller-supplied
/// metadata URI or website origin is never proof of authorization.
pub fn require_launch_authorization(
    authority: TokenAuthorityOption,
    partner: Pubkey,
    signers: impl IntoIterator<Item = (Pubkey, bool)>,
) -> Result<()> {
    if matches!(
        authority,
        TokenAuthorityOption::PartnerUpdateAuthority
            | TokenAuthorityOption::PartnerUpdateAndMintAuthority
    ) {
        require!(
            signers
                .into_iter()
                .any(|(key, signed)| signed && key == partner),
            PoolError::Unauthorized
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_does_not_require_platform() {
        for authority in [
            TokenAuthorityOption::CreatorUpdateAuthority,
            TokenAuthorityOption::Immutable,
        ] {
            assert!(require_launch_authorization(authority, Pubkey::new_unique(), []).is_ok());
        }
    }

    #[test]
    fn partner_launch_requires_the_actual_partner_signature() {
        let partner = Pubkey::new_unique();
        for authority in [
            TokenAuthorityOption::PartnerUpdateAuthority,
            TokenAuthorityOption::PartnerUpdateAndMintAuthority,
        ] {
            assert!(require_launch_authorization(authority, partner, []).is_err());
            assert!(require_launch_authorization(authority, partner, [(partner, false)]).is_err());
            assert!(require_launch_authorization(
                authority,
                partner,
                [(Pubkey::new_unique(), true)]
            )
            .is_err());
            assert!(require_launch_authorization(authority, partner, [(partner, true)]).is_ok());
        }
    }
}
