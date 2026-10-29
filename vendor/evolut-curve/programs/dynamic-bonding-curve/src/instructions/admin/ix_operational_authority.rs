use anchor_lang::prelude::*;
use crate::{state::OperationalAuthority, PoolError};

#[derive(Accounts)]
pub struct InitializeOperationalAuthorityCtx<'info> {
    #[account(init, payer = admin, space = 8 + OperationalAuthority::INIT_SPACE,
        seeds = [OperationalAuthority::SEED], bump)]
    pub operational_authority: Account<'info, OperationalAuthority>,
    #[account(mut, constraint = super::assert_eq_admin(admin.key()) @ PoolError::InvalidAdmin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetOperationalAuthorityCtx<'info> {
    #[account(mut, seeds = [OperationalAuthority::SEED], bump)]
    pub operational_authority: Account<'info, OperationalAuthority>,
    #[account(constraint = super::assert_eq_admin(admin.key()) @ PoolError::InvalidAdmin)]
    pub admin: Signer<'info>,
}

fn validate_operator(operator: Pubkey) -> Result<()> {
    // Zero is an explicit revocation. No instruction accepts it as a signer.
    require!(operator != super::treasury::ID && !super::assert_eq_admin(operator), PoolError::InvalidInput);
    Ok(())
}

pub fn handle_initialize_operational_authority(
    ctx: Context<InitializeOperationalAuthorityCtx>, operator: Pubkey,
) -> Result<()> {
    validate_operator(operator)?;
    ctx.accounts.operational_authority.operator = operator;
    Ok(())
}

pub fn handle_set_operational_authority(
    ctx: Context<SetOperationalAuthorityCtx>, operator: Pubkey,
) -> Result<()> {
    validate_operator(operator)?;
    ctx.accounts.operational_authority.operator = operator;
    Ok(())
}
