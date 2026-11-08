//! Atomic conversion of newly claimed fees only. No LP principal, existing wallet
//! tokens, or existing wrapped SOL can be consumed by these instructions.
use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token::{self, Mint, Token, TokenAccount}};
use crate::{const_pda, raydium::*, state::{PoolConfig, VirtualPool, RaydiumCreatorFees}, PoolError,
    instructions::{ClaimRaydiumCreatorFeesCtx, claim_raydium_creator_fees_inner_limited}};

pub const FEE_SOL_0: &[u8] = b"fee_sol_0";
pub const FEE_SOL_1: &[u8] = b"fee_sol_1";

#[event]
pub struct SolFeeClaimed {
    pub claimant: Pubkey,
    pub virtual_pool: Pubkey,
    pub raydium_pool: Pubkey,
    pub source_kind: u8,
    pub side: u8,
    pub claimed_base: u64,
    pub claimed_quote: u64,
    /// Native SOL delivered, excluding temporary-account rent refunds and network fees.
    pub sol_out: u64,
    pub nonce: [u8; 16],
}

#[derive(Accounts)]
#[instruction(side: u8, min_sol_out: u64, nonce: [u8;16])]
pub struct ClaimCurveFeesSolCtx<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    /// CHECK: Canonical curve vault signer, never a user-controlled CPI target.
    #[account(address = const_pda::pool_authority::ID)]
    pub pool_authority: UncheckedAccount<'info>,
    #[account(has_one = quote_mint)]
    pub config: AccountLoader<'info, PoolConfig>,
    #[account(mut, has_one = config, has_one = quote_vault)]
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    #[account(address = token::spl_token::native_mint::ID)]
    pub quote_mint: Account<'info, Mint>,
    #[account(mut, token::mint = quote_mint, token::authority = pool_authority)]
    pub quote_vault: Account<'info, TokenAccount>,
    #[account(init, payer = owner, seeds = [FEE_SOL_1, virtual_pool.key().as_ref(), owner.key().as_ref(), &nonce],
        bump, token::mint = quote_mint, token::authority = owner)]
    pub temporary_quote: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_claim_curve_fees_sol(ctx: Context<ClaimCurveFeesSolCtx>, side: u8, min_sol_out: u64, nonce: [u8;16]) -> Result<()> {
    require!(side < 2 && min_sol_out > 0, PoolError::InvalidInput);
    let a = &mut *ctx.accounts;
    // An externally prefunded WSOL PDA must not be counted as newly claimed fees.
    require!(a.temporary_quote.amount == 0, PoolError::InvalidAccount);
    let config = a.config.load()?;
    let mut pool = a.virtual_pool.load_mut()?;
    // Old output-token fee configurations require an actual conversion route;
    // never silently return tokens or partially clear their accrued balances.
    let amount = if side == 0 {
        require_keys_eq!(a.owner.key(), pool.creator, PoolError::Unauthorized);
        require!(pool.creator_base_fee == 0, PoolError::InvalidInput);
        pool.claim_creator_trading_fee(0, u64::MAX)?.1
    } else {
        require_keys_eq!(a.owner.key(), config.fee_claimer, PoolError::Unauthorized);
        require_keys_eq!(a.owner.key(), crate::treasury::ID, PoolError::Unauthorized);
        require!(pool.partner_base_fee == 0, PoolError::InvalidInput);
        pool.claim_partner_trading_fee(0, u64::MAX)?.1
    };
    require!(amount >= min_sol_out, PoolError::ExceededSlippage);
    drop(pool); drop(config);
    let bump = [const_pda::pool_authority::BUMP];
    token::transfer(CpiContext::new_with_signer(a.token_program.to_account_info(), token::Transfer {
        from: a.quote_vault.to_account_info(), to: a.temporary_quote.to_account_info(), authority: a.pool_authority.to_account_info(),
    }, &[&[b"pool_authority", &bump]]), amount)?;
    close_token(&a.token_program, &a.temporary_quote.to_account_info(), &a.owner)?;
    emit!(SolFeeClaimed { claimant:a.owner.key(), virtual_pool:a.virtual_pool.key(), raydium_pool:Pubkey::default(),
        source_kind:side, side, claimed_base:0, claimed_quote:amount, sol_out:amount, nonce });
    Ok(())
}

#[derive(Accounts)]
#[instruction(side: u8, min_sol_out: u64, nonce: [u8;16])]
pub struct ClaimRaydiumLpFeesSolCtx<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(has_one = config)]
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    pub config: AccountLoader<'info, PoolConfig>,
    #[account(mut, token::authority = owner, constraint = fee_nft_account.amount == 1 @ PoolError::InvalidAccount)]
    pub fee_nft_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: Canonical graduation receipt and decoded official locker checked in handler.
    #[account(mut)]
    pub fee_lock: UncheckedAccount<'info>,
    /// CHECK: Exact official executable only.
    #[account(address = LOCK_ID, executable)]
    pub lock_program: UncheckedAccount<'info>,
    /// CHECK: Official locker PDA checked in handler.
    pub lock_authority: UncheckedAccount<'info>,
    /// CHECK: Exact official executable only.
    #[account(address = CPMM_ID, executable)]
    pub cpmm_program: UncheckedAccount<'info>,
    /// CHECK: Official CPMM PDA checked in handler.
    pub cpmm_authority: UncheckedAccount<'info>,
    /// CHECK: Receipt, owner and decoded official pool checked in handler.
    #[account(mut)]
    pub raydium_pool: UncheckedAccount<'info>,
    /// CHECK: Bound to pool and original config; Raydium validates its state.
    #[account(owner = CPMM_ID)]
    pub raydium_amm_config: UncheckedAccount<'info>,
    #[account(mut)]
    pub lp_mint: Box<Account<'info, Mint>>,
    pub token_0_mint: Box<Account<'info, Mint>>,
    pub token_1_mint: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = token_0_mint, token::authority = cpmm_authority)]
    pub token_0_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = token_1_mint, token::authority = cpmm_authority)]
    pub token_1_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = lp_mint, token::authority = lock_authority)]
    pub locked_lp_vault: Box<Account<'info, TokenAccount>>,
    #[account(init, payer = owner, seeds = [FEE_SOL_0, virtual_pool.key().as_ref(), owner.key().as_ref(), &nonce],
        bump, token::mint = token_0_mint, token::authority = owner)]
    pub temporary_0: Box<Account<'info, TokenAccount>>,
    #[account(init, payer = owner, seeds = [FEE_SOL_1, virtual_pool.key().as_ref(), owner.key().as_ref(), &nonce],
        bump, token::mint = token_1_mint, token::authority = owner)]
    pub temporary_1: Box<Account<'info, TokenAccount>>,
    /// CHECK: Bound to decoded official pool.
    #[account(mut, owner = CPMM_ID)]
    pub observation: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Fixed Token-2022 executable required by official locker ABI.
    #[account(address = anchor_spl::token_2022::ID, executable)]
    pub token_2022_program: UncheckedAccount<'info>,
    /// CHECK: Fixed Memo executable required by official locker ABI.
    #[account(address = anchor_lang::solana_program::pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"), executable)]
    pub memo_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_claim_raydium_lp_fees_sol(ctx: Context<ClaimRaydiumLpFeesSolCtx>, side:u8, min_sol_out:u64, nonce:[u8;16]) -> Result<()> {
    claim_lp_inner(ctx,side,min_sol_out,nonce,u64::MAX,None)
}
pub fn handle_settle_buyback_lp_fees(ctx:Context<ClaimRaydiumLpFeesSolCtx>,side:u8,min_sol_out:u64,nonce:[u8;16],max_claim_amount:u64,max_impact_bps:u16,max_slippage_bps:u16)->Result<()>{
    require_buyback_owner(ctx.accounts.owner.key(),ctx.accounts.virtual_pool.key(),side,max_impact_bps,max_slippage_bps)?;
    require!(max_claim_amount>0,PoolError::InvalidInput);
    claim_lp_inner(ctx,side,min_sol_out,nonce,max_claim_amount,Some((max_impact_bps,max_slippage_bps)))
}
fn claim_lp_inner(ctx:Context<ClaimRaydiumLpFeesSolCtx>,side:u8,min_sol_out:u64,nonce:[u8;16],max_claim_amount:u64,buyback_guard:Option<(u16,u16)>)->Result<()>{
    require!(side < 2 && (min_sol_out > 0||buyback_guard.is_some()), PoolError::InvalidInput);
    let a = &mut *ctx.accounts;
    require!(a.temporary_0.amount == 0 && a.temporary_1.amount == 0, PoolError::InvalidAccount);
    let pool = a.virtual_pool.load()?;
    let config = a.config.load()?;
    require!(pool.is_migrated == 1, PoolError::InvalidInput);
    let receipt = &pool.graduation;
    let (nft, fee_lock) = if side == 0 {(receipt.creator_fee_nft,receipt.creator_lock)} else {(receipt.partner_fee_nft,receipt.partner_lock)};
    require_keys_eq!(a.fee_nft_account.mint,nft,PoolError::InvalidAccount);
    require_keys_eq!(a.fee_lock.key(),fee_lock,PoolError::InvalidAccount);
    require_keys_eq!(a.raydium_pool.key(),receipt.raydium_pool,PoolError::InvalidAccount);
    require_keys_eq!(a.lp_mint.key(),receipt.lp_mint,PoolError::InvalidAccount);
    require_keys_eq!(a.raydium_amm_config.key(),config.raydium_amm_config,PoolError::InvalidAccount);
    require_pda(&a.fee_lock,&LOCK_ID,&[b"locked_liquidity",nft.as_ref()])?;
    require_pda(&a.lock_authority,&LOCK_ID,&[b"lock_cp_authority_seed"])?;
    require_ata(&a.locked_lp_vault.to_account_info(),&a.lock_authority.key(),&a.lp_mint.key())?;
    let locked = decode_lock(&a.fee_lock)?;
    require!(locked.pool_id == receipt.raydium_pool && locked.fee_nft_mint == nft && locked.lp_mint == receipt.lp_mint,PoolError::InvalidAccount);
    let cp = decode_pool(&a.raydium_pool)?;
    check_cpmm(&cp, &a.cpmm_authority, &a.raydium_amm_config, &a.token_0_mint, &a.token_1_mint,
        &a.token_0_vault, &a.token_1_vault, &a.observation, pool.base_mint, config.quote_mint)?;
    require_keys_eq!(cp.lp_mint,a.lp_mint.key(),PoolError::InvalidAccount);
    drop(pool); drop(config);
    // The locker always CPIs CPMM Withdraw, even when accrued growth rounds to
    // zero on one side; CPMM then rejects ZeroTradingTokens. A keeper still needs
    // a fresh zero-fee observation before freezing a milestone. Only bypass the
    // CPI when a conservative bound on ALL accrued LP fees cannot withdraw one
    // whole unit of both assets. Never suppress a failed CPI or use the caller's
    // partial-claim cap to manufacture a zero observation.
    if buyback_guard.is_some() && min_sol_out == 0 && lp_fees_are_withdrawal_dust(
        &locked, &cp, a.token_0_vault.amount, a.token_1_vault.amount)? {
        close_token(&a.token_program,&a.temporary_0.to_account_info(),&a.owner)?;
        close_token(&a.token_program,&a.temporary_1.to_account_info(),&a.owner)?;
        emit!(SolFeeClaimed {claimant:a.owner.key(),virtual_pool:a.virtual_pool.key(),raydium_pool:a.raydium_pool.key(),
            source_kind:2,side,claimed_base:0,claimed_quote:0,sol_out:0,nonce});
        return Ok(());
    }
    let infos = [a.lock_authority.to_account_info(), a.owner.to_account_info(), a.fee_nft_account.to_account_info(),
        a.fee_lock.to_account_info(), a.cpmm_program.to_account_info(), a.cpmm_authority.to_account_info(),
        a.raydium_pool.to_account_info(), a.lp_mint.to_account_info(), a.temporary_0.to_account_info(),a.temporary_1.to_account_info(),
        a.token_0_vault.to_account_info(),a.token_1_vault.to_account_info(),a.token_0_mint.to_account_info(),a.token_1_mint.to_account_info(),
        a.locked_lp_vault.to_account_info(),a.token_program.to_account_info(),a.token_2022_program.to_account_info(),a.memo_program.to_account_info()];
    let ordered:Vec<_>=infos.iter().enumerate().map(|(i,v)|(v,matches!(i,2|3|6|7|8|9|10|11|14),i==1)).collect();
    let mut data=vec![8,30,51,199,209,184,247,133];data.extend_from_slice(&max_claim_amount.to_le_bytes());
    invoke_abi(&a.lock_program,&ordered,data,&[])?;
    // Burn & Earn redeems only accrued growth by burning the corresponding LP
    // units. Its fixed official CollectCpFees path cannot withdraw principal;
    // do not incorrectly require the remaining LP unit count to stay constant.
    let after_lock = decode_lock(&a.fee_lock)?;
    require!(after_lock.pool_id == locked.pool_id && after_lock.fee_nft_mint == locked.fee_nft_mint
        && after_lock.lp_mint == locked.lp_mint && after_lock.locked_owner == locked.locked_owner
        && (after_lock.locked_lp_amount as u128 + after_lock.claimed_lp_amount as u128)
            == (locked.locked_lp_amount as u128 + locked.claimed_lp_amount as u128)
        && after_lock.locked_lp_amount > 0
        && after_lock.locked_lp_amount <= locked.locked_lp_amount, PoolError::InvalidAccount);
    a.temporary_0.reload()?;a.temporary_1.reload()?;
    let claimed=[a.temporary_0.amount,a.temporary_1.amount];
    let sol=convert_and_close(&a.owner,&a.cpmm_program,&a.cpmm_authority,&a.raydium_amm_config,&a.raydium_pool,
        &a.token_0_mint,&a.token_1_mint,&a.token_0_vault,&a.token_1_vault,&a.observation,&a.token_program,
        &mut a.temporary_0,&mut a.temporary_1,min_sol_out,buyback_guard)?;
    let base_first=a.token_1_mint.key()==token::spl_token::native_mint::ID;
    emit!(SolFeeClaimed {claimant:a.owner.key(),virtual_pool:a.virtual_pool.key(),raydium_pool:a.raydium_pool.key(),
        source_kind:2,side,claimed_base:claimed[if base_first {0}else{1}],claimed_quote:claimed[if base_first {1}else{0}],sol_out:sol,nonce});
    Ok(())
}

/// Bound fee growth without reproducing the locker's payout rounding. LP value
/// grows with sqrt(K)/LP supply. Using ceil(sqrt(current K)) and floor(sqrt(last
/// K)) overstates that growth; charging it against all remaining locked units
/// (including pending fees), then rounding up and adding two units, also exceeds
/// the redeemable growth. Thus a zero reserve output under this bound proves
/// that the official two-asset withdrawal cannot currently succeed. No lock
/// state or fee entitlement is cleared: dust remains for the next collection.
fn lp_fees_are_withdrawal_dust(lock:&LockedLiquidity,pool:&CpmmPool,amount0:u64,amount1:u64)->Result<bool>{
    use ruint::aliases::U256;
    use crate::math::{utils_math::sqrt_u256,u128x128_math::{mul_div_u256,Rounding}};
    if pool.lp_supply==0 || lock.last_lp_amount==0 || lock.last_k==0 || lock.locked_lp_amount==0 {return Ok(false);}
    let reserves=effective_reserves(pool,amount0,amount1)?;
    let current_k=U256::from(reserves[0])*U256::from(reserves[1]);
    let current_root=sqrt_u256(current_k).ok_or(PoolError::MathOverflow)?;
    let last_root=sqrt_u256(U256::from(lock.last_k)).ok_or(PoolError::MathOverflow)?;
    if last_root==U256::ZERO {return Ok(false);}
    let upper_current=(current_root+U256::ONE)*U256::from(lock.last_lp_amount);
    let lower_previous=last_root*U256::from(pool.lp_supply);
    let growth=if upper_current>lower_previous {
        mul_div_u256(U256::from(lock.locked_lp_amount),upper_current-lower_previous,lower_previous,Rounding::Up)
            .ok_or(PoolError::MathOverflow)?
    } else {U256::ZERO};
    let bound=(growth+U256::from(lock.unclaimed_lp_amount)+U256::from(2u8)).min(U256::from(lock.locked_lp_amount));
    Ok(reserves.iter().any(|reserve|bound*U256::from(*reserve)<U256::from(pool.lp_supply)))
}

#[derive(Accounts)]
#[instruction(side:u8,min_sol_out:u64,nonce:[u8;16])]
pub struct ClaimRaydiumCreatorFeesSolCtx<'info> {

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
    #[account(init,payer=owner,seeds=[FEE_SOL_0,virtual_pool.key().as_ref(),owner.key().as_ref(),&nonce],
        bump,token::mint=token_0_mint,token::authority=owner,token::token_program=token_program)]
    pub temporary_0: Box<Account<'info,TokenAccount>>,
    #[account(init,payer=owner,seeds=[FEE_SOL_1,virtual_pool.key().as_ref(),owner.key().as_ref(),&nonce],
        bump,token::mint=token_1_mint,token::authority=owner,token::token_program=token_program)]
    pub temporary_1: Box<Account<'info,TokenAccount>>,
    /// CHECK: Bound to decoded pool before collection.
    #[account(mut,owner=CPMM_ID)]
    pub observation: UncheckedAccount<'info>,
}

pub fn handle_claim_raydium_creator_fees_sol(ctx:Context<ClaimRaydiumCreatorFeesSolCtx>,side:u8,min_sol_out:u64,nonce:[u8;16])->Result<()> {
    claim_creator_inner(ctx,side,min_sol_out,nonce,u64::MAX,None)
}
pub fn handle_settle_buyback_creator_fees(ctx:Context<ClaimRaydiumCreatorFeesSolCtx>,side:u8,min_sol_out:u64,nonce:[u8;16],max_claim_amount:u64,max_impact_bps:u16,max_slippage_bps:u16)->Result<()>{
    require_buyback_owner(ctx.accounts.owner.key(),ctx.accounts.virtual_pool.key(),side,max_impact_bps,max_slippage_bps)?;
    require!(max_claim_amount>0,PoolError::InvalidInput);
    claim_creator_inner(ctx,side,min_sol_out,nonce,max_claim_amount,Some((max_impact_bps,max_slippage_bps)))
}
fn claim_creator_inner(ctx:Context<ClaimRaydiumCreatorFeesSolCtx>,side:u8,min_sol_out:u64,nonce:[u8;16],max_claim_amount:u64,buyback_guard:Option<(u16,u16)>)->Result<()>{
    require!(side<2 && (min_sol_out>0||buyback_guard.is_some()),PoolError::InvalidInput);
    let a=&mut *ctx.accounts;
    require!(a.temporary_0.amount == 0 && a.temporary_1.amount == 0, PoolError::InvalidAccount);
    let cp=decode_pool(&a.raydium_pool)?;
    let pool=a.virtual_pool.load()?;let config=a.config.load()?;
    check_cpmm(&cp,&a.cpmm_authority,&a.raydium_amm_config,&a.token_0_mint,&a.token_1_mint,
        &a.token_0_vault,&a.token_1_vault,&a.observation,pool.base_mint,config.quote_mint)?;
    drop(pool);drop(config);
    let before=[a.destination_0.amount,a.destination_1.amount];
    let mut claim = ClaimRaydiumCreatorFeesCtx {
        owner: a.owner.clone(),
        virtual_pool: a.virtual_pool.clone(),
        config: a.config.clone(),
        fee_authority: a.fee_authority.clone(),
        ledger: a.ledger.clone(),
        fee_nft_account: a.fee_nft_account.clone(),
        fee_lock: a.fee_lock.clone(),
        raydium_pool: a.raydium_pool.clone(),
        raydium_amm_config: a.raydium_amm_config.clone(),
        cpmm_program: a.cpmm_program.clone(),
        cpmm_authority: a.cpmm_authority.clone(),
        token_0_mint: a.token_0_mint.clone(),
        token_1_mint: a.token_1_mint.clone(),
        token_0_vault: a.token_0_vault.clone(),
        token_1_vault: a.token_1_vault.clone(),
        collected_0: a.collected_0.clone(),
        collected_1: a.collected_1.clone(),
        destination_0: a.destination_0.clone(),
        destination_1: a.destination_1.clone(),
        creator_fee_share: a.creator_fee_share.clone(),
        token_program: a.token_program.clone(),
        associated_token_program: a.associated_token_program.clone(),
        system_program: a.system_program.clone(),
    };
    claim_raydium_creator_fees_inner_limited(&mut claim,side,ctx.bumps.fee_authority,max_claim_amount,buyback_guard)?;
    // The legacy ledger is copied back so Anchor serializes its updated entitlement.
    a.ledger.set_inner(claim.ledger.into_inner());
    a.destination_0.reload()?;a.destination_1.reload()?;
    let claimed=[a.destination_0.amount.checked_sub(before[0]).ok_or(PoolError::MathOverflow)?,
        a.destination_1.amount.checked_sub(before[1]).ok_or(PoolError::MathOverflow)?];
    for (source,dest,amount) in [(&a.destination_0,&a.temporary_0,claimed[0]),(&a.destination_1,&a.temporary_1,claimed[1])] {
        if amount>0 {token::transfer(CpiContext::new(a.token_program.to_account_info(),token::Transfer {
            from:source.to_account_info(),to:dest.to_account_info(),authority:a.owner.to_account_info(),}),amount)?;}
    }
    a.temporary_0.reload()?;a.temporary_1.reload()?;
    let sol=convert_and_close(&a.owner,&a.cpmm_program,&a.cpmm_authority,&a.raydium_amm_config,&a.raydium_pool,
        &a.token_0_mint,&a.token_1_mint,&a.token_0_vault,&a.token_1_vault,&a.observation,&a.token_program,
        &mut a.temporary_0,&mut a.temporary_1,min_sol_out,buyback_guard)?;
    a.destination_0.reload()?;a.destination_1.reload()?;
    require!([a.destination_0.amount,a.destination_1.amount]==before,PoolError::InvalidAccount);
    let base_first=a.token_1_mint.key()==token::spl_token::native_mint::ID;
    emit!(SolFeeClaimed {claimant:a.owner.key(),virtual_pool:a.virtual_pool.key(),raydium_pool:a.raydium_pool.key(),
        source_kind:3,side,claimed_base:claimed[if base_first {0}else{1}],claimed_quote:claimed[if base_first {1}else{0}],sol_out:sol,nonce});
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_cpmm(cp:&CpmmPool,authority:&AccountInfo,config:&AccountInfo,m0:&Account<Mint>,m1:&Account<Mint>,
    v0:&Account<TokenAccount>,v1:&Account<TokenAccount>,observation:&AccountInfo,base:Pubkey,quote:Pubkey)->Result<()> {
    require_keys_eq!(quote,token::spl_token::native_mint::ID,PoolError::InvalidAccount);
    require!(base!=quote && ((m0.key()==base && m1.key()==quote)||(m1.key()==base && m0.key()==quote)),PoolError::InvalidAccount);
    require!(cp.amm_config==config.key() && cp.token_0_program==token::ID && cp.token_1_program==token::ID
        && cp.token_0_mint==m0.key() && cp.token_1_mint==m1.key() && cp.token_0_vault==v0.key()
        && cp.token_1_vault==v1.key() && cp.observation_key==observation.key(),PoolError::InvalidAccount);
    require_pda(authority,&CPMM_ID,&[b"vault_and_lp_mint_auth_seed"])
}

#[allow(clippy::too_many_arguments)]
fn convert_and_close<'info>(owner:&Signer<'info>,cpmm:&AccountInfo<'info>,authority:&AccountInfo<'info>,config:&AccountInfo<'info>,pool:&AccountInfo<'info>,
    m0:&Account<'info,Mint>,m1:&Account<'info,Mint>,v0:&Account<'info,TokenAccount>,v1:&Account<'info,TokenAccount>,observation:&AccountInfo<'info>,
    token_program:&Program<'info,Token>,temp0:&mut Account<'info,TokenAccount>,temp1:&mut Account<'info,TokenAccount>,min_sol_out:u64,buyback_guard:Option<(u16,u16)>)->Result<u64> {
    let base_first=m1.key()==token::spl_token::native_mint::ID;
    let (base,quote,base_mint,quote_mint,base_vault,quote_vault)=if base_first {(&mut *temp0,&mut *temp1,m0,m1,v0,v1)} else {(&mut *temp1,&mut *temp0,m1,m0,v1,v0)};
    let base_amount=base.amount;
    let mut min_sol_out=min_sol_out;
    if let Some((impact,slippage))=buyback_guard {
        if base_amount>0 {
            let cp=decode_pool(pool)?;let fee=decode_trading_config(config)?;
            // LP collection just changed the official vaults through CPI; the
            // Anchor Account cache still contains their pre-collection amounts.
            let amount0=TokenAccount::try_deserialize(&mut v0.to_account_info().try_borrow_data()?.as_ref())?.amount;
            let amount1=TokenAccount::try_deserialize(&mut v1.to_account_info().try_borrow_data()?.as_ref())?.amount;
            let reserves=effective_reserves(&cp,amount0,amount1)?;
            let input=if base_first{0}else{1};
            let floor=bounded_swap_floor(base_amount,reserves[input],reserves[1-input],&cp,&fee,impact,slippage)?;
            min_sol_out=min_sol_out.max(quote.amount.checked_add(floor).ok_or(PoolError::MathOverflow)?);
        }
    }
    if base_amount>0 {
        let before=quote.amount;
        let infos=[owner.to_account_info(),authority.clone(),config.clone(),pool.clone(),base.to_account_info(),quote.to_account_info(),
            base_vault.to_account_info(),quote_vault.to_account_info(),token_program.to_account_info(),token_program.to_account_info(),
            base_mint.to_account_info(),quote_mint.to_account_info(),observation.clone()];
        let ordered:Vec<_>=infos.iter().enumerate().map(|(i,v)|(v,matches!(i,3|4|5|6|7|12),i==0)).collect();
        let mut data=vec![143,190,90,218,196,30,51,222]; // global:swap_base_input
        data.extend_from_slice(&base_amount.to_le_bytes());
        // The caller bounds total delivered SOL. Do not demand an extra lamport
        // when the claim's quote side alone meets the requested minimum.
        data.extend_from_slice(&min_sol_out.saturating_sub(before).to_le_bytes());
        invoke_abi(cpmm,&ordered,data,&[])?;
        base.reload()?;quote.reload()?;
    }
    require!(base.amount==0 && quote.amount>=min_sol_out,PoolError::ExceededSlippage);
    let output=quote.amount;
    close_token(token_program,&base.to_account_info(),owner)?;
    close_token(token_program,&quote.to_account_info(),owner)?;
    Ok(output)
}

fn require_buyback_owner(owner:Pubkey,pool:Pubkey,side:u8,impact:u16,slippage:u16)->Result<()>{
    require!(side==1&&impact>0&&impact<=100&&slippage>0&&slippage<=100,PoolError::InvalidInput);
    require_keys_eq!(owner,Pubkey::find_program_address(&[crate::instructions::buyback::VAULT_SEED,pool.as_ref()],&crate::ID).0,PoolError::Unauthorized);Ok(())
}

fn close_token<'info>(program:&Program<'info,Token>,account:&AccountInfo<'info>,owner:&Signer<'info>)->Result<()> {
    token::close_account(CpiContext::new(program.to_account_info(),token::CloseAccount {
        account:account.clone(),destination:owner.to_account_info(),authority:owner.to_account_info(),
    }))
}
