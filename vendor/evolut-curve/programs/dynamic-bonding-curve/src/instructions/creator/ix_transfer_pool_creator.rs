use anchor_lang::prelude::*;

use crate::{
    state::{MigrationProgress, PoolConfig, VirtualPool},
    EvtUpdatePoolCreator, PoolError,
};

/// Accounts for transfer pool creator
#[event_cpi]
#[derive(Accounts)]
pub struct TransferPoolCreatorCtx<'info> {
    #[account(
        mut,
        has_one = config,
        has_one = creator,
    )]
    pub virtual_pool: AccountLoader<'info, VirtualPool>,

    pub config: AccountLoader<'info, PoolConfig>,

    pub creator: Signer<'info>,

    /// CHECK: new creator address, can be anything except old creator
    #[account(
        constraint = new_creator.key().ne(creator.key) @ PoolError::InvalidNewCreator,
    )]
    pub new_creator: UncheckedAccount<'info>,
}

pub fn handle_transfer_pool_creator<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, TransferPoolCreatorCtx>,
) -> Result<()> {
    let mut pool = ctx.accounts.virtual_pool.load_mut()?;

    let migration_progress = pool.get_migration_progress()?;
    require!(
        matches!(
            migration_progress,
            MigrationProgress::PreBondingCurve | MigrationProgress::CreatedPool
        ),
        PoolError::NotPermitToDoThisAction
    );
    // The immutable graduation receipt records the initial beneficiary; current fee-NFT ownership
    // is independent of transferring the residual curve creator role.
    pool.creator = ctx.accounts.new_creator.key();

    emit_cpi!(EvtUpdatePoolCreator {
        pool: ctx.accounts.virtual_pool.key(),
        creator: ctx.accounts.creator.key(),
        new_creator: ctx.accounts.new_creator.key(),
    });
    Ok(())
}
