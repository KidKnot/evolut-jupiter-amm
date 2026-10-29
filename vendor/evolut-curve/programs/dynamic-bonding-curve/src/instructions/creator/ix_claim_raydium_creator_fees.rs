//! Creator fees are separate from locked-LP income. Each pool has its own PDA
//! ATAs so permissionless external collection cannot mix funds between pools.
use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token::{self, Mint, Token, TokenAccount}};
use crate::{raydium::*, state::{PoolConfig, VirtualPool, RaydiumCreatorFees}, PoolError};

#[derive(Accounts)]
pub struct ClaimRaydiumCreatorFeesCtx<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(has_one = config)]
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    pub config: AccountLoader<'info, PoolConfig>,
    /// CHECK: canonical per-pool signer; never stores a private key.
    #[account(seeds = [CREATOR_AUTH_SEED, virtual_pool.key().as_ref()], bump)]
    pub fee_authority: UncheckedAccount<'info>,
    #[account(init_if_needed, payer = owner, space = 8 + RaydiumCreatorFees::INIT_SPACE,
        seeds = [RaydiumCreatorFees::SEED, virtual_pool.key().as_ref()], bump)]
    pub ledger: Account<'info, RaydiumCreatorFees>,
    /// Current ownership, not the initial NFT recipient or a token delegate.
    #[account(token::authority = owner, constraint = fee_nft_account.amount == 1 @ PoolError::InvalidAccount)]
    pub fee_nft_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: receipt binding and official locker layout checked in handler.
    pub fee_lock: UncheckedAccount<'info>,
    /// CHECK: receipt binding and official CPMM layout checked in handler.
    #[account(mut)]
    pub raydium_pool: UncheckedAccount<'info>,
    /// CHECK: bound to original PoolConfig and CPMM pool, owned by CPMM.
    #[account(owner = CPMM_ID)]
    pub raydium_amm_config: UncheckedAccount<'info>,
    /// CHECK: official executable only.
    #[account(address = CPMM_ID, executable)]
    pub cpmm_program: UncheckedAccount<'info>,
    /// CHECK: official PDA checked in handler and CPI.
    pub cpmm_authority: UncheckedAccount<'info>,
    pub token_0_mint: Box<Account<'info, Mint>>,
    pub token_1_mint: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = token_0_mint, token::authority = cpmm_authority)]
    pub token_0_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = token_1_mint, token::authority = cpmm_authority)]
    pub token_1_vault: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = owner, associated_token::mint = token_0_mint,
        associated_token::authority = fee_authority)]
    pub collected_0: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = owner, associated_token::mint = token_1_mint,
        associated_token::authority = fee_authority)]
    pub collected_1: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = owner, associated_token::mint = token_0_mint,
        associated_token::authority = owner)]
    pub destination_0: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = owner, associated_token::mint = token_1_mint,
        associated_token::authority = owner)]
    pub destination_1: Box<Account<'info, TokenAccount>>,
    /// CHECK: Raydium verifies canonical creator/config override PDA, even absent.
    pub creator_fee_share: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_claim_raydium_creator_fees(ctx: Context<ClaimRaydiumCreatorFeesCtx>, side: u8) -> Result<()> {
    require!(cfg!(feature = "mainnet") && side < 2, PoolError::InvalidInput);
    let a = &mut *ctx.accounts;
    let pool = a.virtual_pool.load()?;
    let config = a.config.load()?;
    require!(pool.is_migrated == 1 && creator_fee_route(&config.raydium_amm_config), PoolError::InvalidInput);
    let receipt = &pool.graduation;
    let (nft, lock_key) = if side == 0 { (receipt.creator_fee_nft, receipt.creator_lock) }
        else { (receipt.partner_fee_nft, receipt.partner_lock) };
    require_keys_eq!(a.fee_nft_account.mint, nft, PoolError::InvalidAccount);
    require_keys_eq!(a.fee_lock.key(), lock_key, PoolError::InvalidAccount);
    require_pda(&a.fee_lock, &LOCK_ID, &[b"locked_liquidity", nft.as_ref()])?;
    let lock = decode_lock(&a.fee_lock)?;
    require!(lock.pool_id == receipt.raydium_pool && lock.fee_nft_mint == nft
        && lock.lp_mint == receipt.lp_mint, PoolError::InvalidAccount);
    require_keys_eq!(a.raydium_pool.key(), receipt.raydium_pool, PoolError::InvalidAccount);
    require_keys_eq!(a.raydium_amm_config.key(), config.raydium_amm_config, PoolError::InvalidAccount);
    let cp = decode_pool(&a.raydium_pool)?;
    require!(cp.pool_creator == a.fee_authority.key() && cp.enable_creator_fee
        && cp.amm_config == config.raydium_amm_config && cp.lp_mint == receipt.lp_mint
        && cp.token_0_program == token::ID && cp.token_1_program == token::ID,
        PoolError::InvalidAccount);
    require_keys_eq!(cp.token_0_mint, a.token_0_mint.key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.token_1_mint, a.token_1_mint.key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.token_0_vault, a.token_0_vault.key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.token_1_vault, a.token_1_vault.key(), PoolError::InvalidAccount);
    require_pda(&a.cpmm_authority, &CPMM_ID, &[b"vault_and_lp_mint_auth_seed"])?;
    // Borrow guards must be dropped before CPI; no account-data alias survives.
    drop(config);
    drop(pool);
    if cp.creator_fees_token_0 != 0 || cp.creator_fees_token_1 != 0 {
        let infos = [a.owner.to_account_info(), a.fee_authority.to_account_info(),
            a.cpmm_authority.to_account_info(), a.raydium_pool.to_account_info(),
            a.token_0_vault.to_account_info(), a.token_1_vault.to_account_info(),
            a.token_0_mint.to_account_info(), a.token_1_mint.to_account_info(),
            a.collected_0.to_account_info(), a.collected_1.to_account_info(),
            a.token_program.to_account_info(), a.token_program.to_account_info(),
            a.associated_token_program.to_account_info(), a.system_program.to_account_info(),
            a.raydium_amm_config.to_account_info(), a.creator_fee_share.to_account_info()];
        let ordered: Vec<_> = infos.iter().enumerate().map(|(i, account)|
            (account, matches!(i, 0 | 3 | 4 | 5 | 8 | 9), i == 0)).collect();
        invoke_abi(&a.cpmm_program, &ordered, vec![202, 202, 34, 83, 226, 122, 145, 229], &[])?;
        a.collected_0.reload()?;
        a.collected_1.reload()?;
    }
    // Includes earlier permissionless collection into these same pool-specific
    // ATAs. Counting only this CPI's delta would strand/lose those fees.
    let amounts = a.ledger.claimable([a.collected_0.amount, a.collected_1.amount], side)?;
    a.ledger.record(side, amounts)?;
    let pool_key = a.virtual_pool.key();
    let bump = [ctx.bumps.fee_authority];
    let seed: &[&[u8]] = &[CREATOR_AUTH_SEED, pool_key.as_ref(), &bump];
    for (source, destination, amount) in [(&a.collected_0, &a.destination_0, amounts[0]),
        (&a.collected_1, &a.destination_1, amounts[1])] {
        if amount > 0 {
            token::transfer(CpiContext::new_with_signer(a.token_program.to_account_info(), token::Transfer {
                from: source.to_account_info(), to: destination.to_account_info(),
                authority: a.fee_authority.to_account_info(),
            }, &[seed]), amount)?;
        }
    }
    emit!(RaydiumCreatorFeesClaimed { virtual_pool: pool_key, owner: a.owner.key(), side,
        token_0_amount: amounts[0], token_1_amount: amounts[1] });
    Ok(())
}

#[event]
pub struct RaydiumCreatorFeesClaimed {
    pub virtual_pool: Pubkey,
    pub owner: Pubkey,
    pub side: u8,
    pub token_0_amount: u64,
    pub token_1_amount: u64,
}
