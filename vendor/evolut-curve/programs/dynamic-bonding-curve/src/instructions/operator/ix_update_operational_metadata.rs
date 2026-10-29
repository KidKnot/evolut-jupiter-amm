use anchor_lang::prelude::*;
use mpl_token_metadata::{accounts::Metadata, types::DataV2};
use crate::{state::{OperationalAuthority, PoolConfig, VirtualPool, TokenAuthorityOption}, PoolError};

#[derive(Accounts)]
pub struct UpdateOperationalMetadataCtx<'info> {
    pub config: AccountLoader<'info, PoolConfig>,
    #[account(has_one = config)]
    pub pool: AccountLoader<'info, VirtualPool>,
    #[account(seeds = [OperationalAuthority::SEED], bump, has_one = operator)]
    pub operational_authority: Account<'info, OperationalAuthority>,
    pub operator: Signer<'info>,
    /// CHECK: Canonical Metaplex PDA and owner checked here; contents checked below.
    #[account(mut, seeds = [b"metadata", mpl_token_metadata::ID.as_ref(), pool.load()?.base_mint.as_ref()],
        bump, seeds::program = mpl_token_metadata::ID, owner = mpl_token_metadata::ID)]
    pub mint_metadata: UncheckedAccount<'info>,
    /// CHECK: Only the fixed executable Metaplex program can receive this CPI.
    #[account(address = mpl_token_metadata::ID, executable)]
    pub metadata_program: UncheckedAccount<'info>,
}

/// Rename only. No arbitrary instruction, URI, authority, mutability, minting,
/// fee destination or token-transfer capability is delegated to the worker.
pub fn handle_update_operational_metadata(ctx: Context<UpdateOperationalMetadataCtx>, name: String) -> Result<()> {
    require!(ctx.accounts.operator.key() != Pubkey::default(), PoolError::Unauthorized);
    require!(name.as_bytes().len() <= 32, PoolError::InvalidInput);
    let config = ctx.accounts.config.load()?;
    require_keys_eq!(config.fee_claimer, crate::admin::treasury::ID, PoolError::Unauthorized);
    require!(matches!(config.get_token_authority()?, TokenAuthorityOption::PartnerUpdateAuthority
        | TokenAuthorityOption::PartnerUpdateAndMintAuthority), PoolError::Unauthorized);
    let metadata = Metadata::from_bytes(&ctx.accounts.mint_metadata.try_borrow_data()?)
        .map_err(|_| error!(PoolError::InvalidInput))?;
    require_keys_eq!(metadata.mint, ctx.accounts.pool.load()?.base_mint, PoolError::InvalidInput);
    require_keys_eq!(metadata.update_authority, ctx.accounts.operational_authority.key(), PoolError::Unauthorized);
    require!(metadata.is_mutable, PoolError::InvalidInput);
    let account = ctx.accounts.operational_authority.to_account_info();
    let metadata_account = ctx.accounts.mint_metadata.to_account_info();
    let program = ctx.accounts.metadata_program.to_account_info();
    let bump = [ctx.bumps.operational_authority];
    mpl_token_metadata::instructions::UpdateMetadataAccountV2CpiBuilder::new(&program)
        .metadata(&metadata_account).update_authority(&account)
        .data(DataV2 { name, symbol: metadata.symbol, uri: metadata.uri,
            seller_fee_basis_points: metadata.seller_fee_basis_points,
            creators: metadata.creators, collection: metadata.collection, uses: metadata.uses })
        .invoke_signed(&[&[OperationalAuthority::SEED, &bump]])?;
    Ok(())
}
