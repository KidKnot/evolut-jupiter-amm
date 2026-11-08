//! Evolut's FeeKey is held by a per-coin PDA; creator rights and LP principal
//! never enter this module. The operator attests the application's committed
//! milestone; this is deliberately NOT an on-chain USD oracle.
use crate::{
    raydium::*,
    state::{
        OperationalAuthority, PoolConfig, RaydiumCreatorFees, TokenAuthorityOption, VirtualPool,
    },
    PoolError,
};
use anchor_lang::{
    prelude::*,
    solana_program::{program::invoke_signed, system_instruction},
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount},
};

pub const POLICY_SEED: &[u8] = b"buyback_policy";
pub const LEDGER_SEED: &[u8] = b"buyback_ledger";
pub const VAULT_SEED: &[u8] = b"buyback_vault";
pub const COLLECTION_FRESH_SLOTS: u64 = 300;
const FINAL_STAGE: u8 = 8;

#[account]
#[derive(InitSpace)]
pub struct BuybackPolicy {
    pub enabled: bool,
    pub paused: bool,
    pub platform_mint: Pubkey,
    pub platform_pool: Pubkey,
    pub platform_config: Pubkey,
    pub max_batch_lamports: u64,
    pub max_impact_bps: u16,
    pub max_slippage_bps: u16,
    pub cooldown_seconds: u32,
    pub action_nonce: u64,
}

#[account]
#[derive(InitSpace, Default)]
pub struct BuybackLedger {
    pub virtual_pool: Pubkey,
    pub source_mint: Pubkey,
    pub raydium_pool: Pubkey,
    pub fee_nft: Pubkey,
    pub is_evolving: bool,
    pub last_stage: u8,
    pub action_nonce: u64,
    pub generation: u64,
    pub total_net_sol: u128,
    pub eligible_net_sol: u128,
    pub allocated_total: u128,
    /// [source coin, platform token], never arbitrary recipient addresses.
    pub pending: [u64; 2],
    pub frozen: [u64; 2],
    pub spent: [u128; 2],
    pub burned: [u128; 2],
    pub treasury_paid: u128,
    pub recovered: u128,
    pub last_collection_slot: [u64; 2],
    pub last_collection_time: [i64; 2],
    pub last_execution_time: [i64; 2],
    /// Keeper-funded permanent system-account rent, never fee revenue/budget.
    pub rent_reserve: u64,
    pub epochs: [BuybackEpoch; 7],
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Default)]
pub struct BuybackEpoch {
    pub stage: u8,
    pub event_hash: [u8; 32],
    pub milestone_slot: u64,
    pub checkpoint_slot: u64,
    pub generation: u64,
    pub budget: [u64; 2],
    pub remaining: [u64; 2],
    pub burned: [u128; 2],
}

fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| error!(PoolError::MathOverflow))
}
fn add128(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b)
        .ok_or_else(|| error!(PoolError::MathOverflow))
}
fn sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b)
        .ok_or_else(|| error!(PoolError::MathOverflow))
}
fn bump_nonce(nonce: &mut u64, expected: u64) -> Result<()> {
    require!(*nonce == expected, PoolError::InvalidInput);
    *nonce = add(*nonce, 1)?;
    Ok(())
}
impl BuybackLedger {
    pub fn reserved(&self) -> Result<u64> {
        add(
            add(self.pending[0], self.pending[1])?,
            add(self.frozen[0], self.frozen[1])?,
        )
    }
    /// Lifetime rounding: splitting receipts into dust transactions cannot avoid
    /// either 10% reservation. Each new lamport is accounted exactly once.
    pub fn allocate(&mut self, net: u64) -> Result<u64> {
        self.total_net_sol = add128(self.total_net_sol, net as u128)?;
        let mut reserved = 0;
        if self.is_evolving && self.last_stage < FINAL_STAGE {
            self.eligible_net_sol = add128(self.eligible_net_sol, net as u128)?;
            let total = self.eligible_net_sol / 5;
            let old = [
                self.allocated_total / 2,
                self.allocated_total - self.allocated_total / 2,
            ];
            let next = [total / 2, total - total / 2];
            for leg in 0..2 {
                let amount = u64::try_from(
                    next[leg]
                        .checked_sub(old[leg])
                        .ok_or(PoolError::MathOverflow)?,
                )
                .map_err(|_| PoolError::MathOverflow)?;
                self.pending[leg] = add(self.pending[leg], amount)?;
                reserved = add(reserved, amount)?;
            }
            self.allocated_total = total;
        }
        let treasury = sub(net, reserved)?;
        self.treasury_paid = add128(self.treasury_paid, treasury as u128)?;
        Ok(treasury)
    }
}

#[event]
pub struct BuybackRecorded {
    pub virtual_pool: Pubkey,
    pub action: u8,
    pub nonce: u64,
    pub stage: u8,
    pub leg: u8,
    pub sol: u64,
    pub tokens: u64,
    pub generation: u64,
}

#[derive(Accounts)]
pub struct InitializeBuybackPolicyCtx<'info> {
    #[account(mut,constraint=crate::admin::assert_eq_admin(admin.key()) @ PoolError::InvalidAdmin)]
    pub admin: Signer<'info>,
    #[account(init,payer=admin,space=8+BuybackPolicy::INIT_SPACE,seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    pub system_program: Program<'info, System>,
}
pub fn handle_initialize_buyback_policy(
    ctx: Context<InitializeBuybackPolicyCtx>,
    max_batch_lamports: u64,
    max_impact_bps: u16,
    max_slippage_bps: u16,
    cooldown_seconds: u32,
) -> Result<()> {
    require!(
        max_batch_lamports > 0
            && max_batch_lamports <= 1_000_000_000
            && max_impact_bps > 0
            && max_impact_bps <= 100
            && max_slippage_bps > 0
            && max_slippage_bps <= 100
            && (30..=86400).contains(&cooldown_seconds),
        PoolError::InvalidInput
    );
    let p = &mut ctx.accounts.policy;
    p.enabled = false;
    p.paused = true;
    p.max_batch_lamports = max_batch_lamports;
    p.max_impact_bps = max_impact_bps;
    p.max_slippage_bps = max_slippage_bps;
    p.cooldown_seconds = cooldown_seconds;
    Ok(())
}
#[derive(Accounts)]
pub struct AdminBuybackPolicyCtx<'info> {
    #[account(constraint=crate::admin::assert_eq_admin(admin.key()) @ PoolError::InvalidAdmin)]
    pub admin: Signer<'info>,
    #[account(mut,seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
}
pub fn handle_set_buyback_policy_status(
    ctx: Context<AdminBuybackPolicyCtx>,
    enabled: bool,
    paused: bool,
    expected_nonce: u64,
) -> Result<()> {
    let p = &mut ctx.accounts.policy;
    require!(!p.enabled || enabled, PoolError::InvalidInput);
    require!(enabled || paused, PoolError::InvalidInput);
    bump_nonce(&mut p.action_nonce, expected_nonce)?;
    p.enabled = enabled;
    p.paused = paused;
    Ok(())
}
#[derive(Accounts)]
pub struct SetBuybackPlatformCtx<'info> {
    #[account(constraint=crate::admin::assert_eq_admin(admin.key()) @ PoolError::InvalidAdmin)]
    pub admin: Signer<'info>,
    #[account(mut,seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    #[account(has_one=config,has_one=base_mint)]
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    pub config: AccountLoader<'info, PoolConfig>,
    pub base_mint: Account<'info, Mint>,
    /// CHECK: Exact receipt and official owner/layout checked below.
    pub raydium_pool: UncheckedAccount<'info>,
}
pub fn handle_set_buyback_platform(
    ctx: Context<SetBuybackPlatformCtx>,
    expected_nonce: u64,
) -> Result<()> {
    let a = &mut *ctx.accounts;
    require!(
        a.policy.platform_mint == Pubkey::default(),
        PoolError::InvalidInput
    );
    let v = a.virtual_pool.load()?;
    let c = a.config.load()?;
    require!(v.is_migrated == 1, PoolError::InvalidInput);
    require_keys_eq!(
        v.graduation.raydium_pool,
        a.raydium_pool.key(),
        PoolError::InvalidAccount
    );
    require_keys_eq!(
        c.fee_claimer,
        crate::treasury::ID,
        PoolError::InvalidAccount
    );
    let cp = decode_pool(&a.raydium_pool)?;
    require_keys_eq!(
        cp.amm_config,
        c.raydium_amm_config,
        PoolError::InvalidAccount
    );
    require!(
        a.base_mint.supply > 0
            && a.base_mint.freeze_authority.is_none()
            && a.base_mint.mint_authority.is_none()
            && ((cp.token_0_mint == v.base_mint
                && cp.token_1_mint == token::spl_token::native_mint::ID)
                || (cp.token_1_mint == v.base_mint
                    && cp.token_0_mint == token::spl_token::native_mint::ID))
            && cp.token_0_program == token::ID
            && cp.token_1_program == token::ID,
        PoolError::InvalidAccount
    );
    bump_nonce(&mut a.policy.action_nonce, expected_nonce)?;
    a.policy.platform_mint = v.base_mint;
    a.policy.platform_pool = a.raydium_pool.key();
    a.policy.platform_config = cp.amm_config;
    Ok(())
}

/// Every upgraded graduate call supplies the canonical policy account, including
/// the still-uninitialized system-owned PDA. Omitting it must never bypass enable.
pub fn graduation_beneficiary(
    account: &AccountInfo,
    pool: Pubkey,
    config: &PoolConfig,
) -> Result<Pubkey> {
    require_pda(account, &crate::ID, &[POLICY_SEED])?;
    if account.owner == &anchor_lang::system_program::ID {
        require!(account.data_is_empty(), PoolError::InvalidAccount);
        return Ok(crate::treasury::ID);
    }
    require_keys_eq!(*account.owner, crate::ID, PoolError::InvalidAccount);
    let p = BuybackPolicy::try_deserialize(&mut account.try_borrow_data()?.as_ref())?;
    let evolving = matches!(
        config.get_token_authority()?,
        TokenAuthorityOption::PartnerUpdateAuthority
            | TokenAuthorityOption::PartnerUpdateAndMintAuthority
    );
    Ok(if p.enabled && evolving {
        Pubkey::find_program_address(&[VAULT_SEED, pool.as_ref()], &crate::ID).0
    } else {
        crate::treasury::ID
    })
}

#[derive(Accounts)]
pub struct AdoptBuybackFeeKeyCtx<'info> {
    #[account(mut,address=crate::treasury::ID)]
    pub treasury: Signer<'info>,
    #[account(seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    /// CHECK: System-owned, zero-data custody signer PDA.
    #[account(seeds=[VAULT_SEED,virtual_pool.key().as_ref()],bump,owner=anchor_lang::system_program::ID)]
    pub vault: UncheckedAccount<'info>,
    #[account(address=virtual_pool.load()?.graduation.partner_fee_nft)]
    pub fee_nft: Account<'info, Mint>,
    #[account(mut,token::mint=fee_nft,token::authority=treasury,constraint=source.amount==1 @ PoolError::InvalidAccount)]
    pub source: Account<'info, TokenAccount>,
    #[account(init_if_needed,payer=treasury,associated_token::mint=fee_nft,associated_token::authority=vault)]
    pub destination: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
pub fn handle_adopt_buyback_fee_key(ctx: Context<AdoptBuybackFeeKeyCtx>) -> Result<()> {
    let a = &ctx.accounts;
    let p = a.virtual_pool.load()?;
    require!(
        a.policy.enabled
            && p.is_migrated == 1
            && a.vault.data_is_empty()
            && a.destination.amount == 0,
        PoolError::InvalidInput
    );
    require_keys_eq!(
        p.graduation.initial_partner,
        crate::treasury::ID,
        PoolError::InvalidAccount
    );
    token::transfer(
        CpiContext::new(
            a.token_program.to_account_info(),
            token::Transfer {
                from: a.source.to_account_info(),
                to: a.destination.to_account_info(),
                authority: a.treasury.to_account_info(),
            },
        ),
        1,
    )
}

#[derive(Accounts)]
pub struct InitializeBuybackPoolCtx<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub operator: Signer<'info>,
    #[account(seeds=[OperationalAuthority::SEED],bump,has_one=operator)]
    pub operational_authority: Account<'info, OperationalAuthority>,
    #[account(seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    #[account(has_one=config)]
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    pub config: AccountLoader<'info, PoolConfig>,
    #[account(init,payer=payer,space=8+BuybackLedger::INIT_SPACE,seeds=[LEDGER_SEED,virtual_pool.key().as_ref()],bump)]
    pub ledger: Box<Account<'info, BuybackLedger>>,
    /// CHECK: Per-pool system-owned signer; donations are not revenue.
    #[account(mut,seeds=[VAULT_SEED,virtual_pool.key().as_ref()],bump,owner=anchor_lang::system_program::ID)]
    pub vault: UncheckedAccount<'info>,
    #[account(token::authority=vault,constraint=fee_nft_account.amount==1 @ PoolError::InvalidAccount)]
    pub fee_nft_account: Box<Account<'info, TokenAccount>>,
    /// CHECK: Receipt/official pool checks below bind both mint identities.
    pub raydium_pool: UncheckedAccount<'info>,
    pub token_0_mint: Box<Account<'info, Mint>>,
    pub token_1_mint: Box<Account<'info, Mint>>,
    /// CHECK: Existing per-pool Raydium collection PDA.
    #[account(seeds=[CREATOR_AUTH_SEED,virtual_pool.key().as_ref()],bump)]
    pub fee_authority: UncheckedAccount<'info>,
    #[account(init_if_needed,payer=payer,space=8+RaydiumCreatorFees::INIT_SPACE,seeds=[RaydiumCreatorFees::SEED,virtual_pool.key().as_ref()],bump)]
    pub creator_ledger: Account<'info, RaydiumCreatorFees>,
    #[account(init_if_needed,payer=payer,associated_token::mint=token_0_mint,associated_token::authority=fee_authority)]
    pub collected_0: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed,payer=payer,associated_token::mint=token_1_mint,associated_token::authority=fee_authority)]
    pub collected_1: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed,payer=payer,associated_token::mint=token_0_mint,associated_token::authority=vault)]
    pub destination_0: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed,payer=payer,associated_token::mint=token_1_mint,associated_token::authority=vault)]
    pub destination_1: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
pub fn handle_initialize_buyback_pool(ctx: Context<InitializeBuybackPoolCtx>) -> Result<()> {
    let a = &mut *ctx.accounts;
    let v = a.virtual_pool.load()?;
    let c = a.config.load()?;
    require!(
        a.policy.enabled && v.is_migrated == 1 && a.vault.data_is_empty(),
        PoolError::InvalidInput
    );
    require_keys_eq!(
        c.fee_claimer,
        crate::treasury::ID,
        PoolError::InvalidAccount
    );
    require_keys_eq!(
        a.fee_nft_account.mint,
        v.graduation.partner_fee_nft,
        PoolError::InvalidAccount
    );
    require!(
        a.fee_nft_account.delegate.is_none() && a.fee_nft_account.close_authority.is_none(),
        PoolError::InvalidAccount
    );
    require_keys_eq!(
        a.raydium_pool.key(),
        v.graduation.raydium_pool,
        PoolError::InvalidAccount
    );
    let cp = decode_pool(&a.raydium_pool)?;
    require!(
        cp.amm_config == c.raydium_amm_config
            && cp.token_0_mint == a.token_0_mint.key()
            && cp.token_1_mint == a.token_1_mint.key()
            && cp.token_0_program == token::ID
            && cp.token_1_program == token::ID
            && ((cp.token_0_mint == v.base_mint
                && cp.token_1_mint == token::spl_token::native_mint::ID)
                || (cp.token_1_mint == v.base_mint
                    && cp.token_0_mint == token::spl_token::native_mint::ID)),
        PoolError::InvalidAccount
    );
    a.ledger.virtual_pool = a.virtual_pool.key();
    a.ledger.source_mint = v.base_mint;
    a.ledger.raydium_pool = a.raydium_pool.key();
    a.ledger.fee_nft = v.graduation.partner_fee_nft;
    a.ledger.is_evolving = matches!(
        c.get_token_authority()?,
        TokenAuthorityOption::PartnerUpdateAuthority
            | TokenAuthorityOption::PartnerUpdateAndMintAuthority
    );
    a.ledger.last_stage = 1;
    a.ledger.action_nonce = 1;
    a.ledger.rent_reserve = Rent::get()?.minimum_balance(0);
    let shortfall = a.ledger.rent_reserve.saturating_sub(a.vault.lamports());
    if shortfall > 0 {
        invoke_signed(
            &system_instruction::transfer(&a.payer.key(), &a.vault.key(), shortfall),
            &[
                a.payer.to_account_info(),
                a.vault.to_account_info(),
                a.system_program.to_account_info(),
            ],
            &[],
        )?;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct CollectBuybackFeesCtx<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(seeds=[OperationalAuthority::SEED],bump,has_one=operator)]
    pub operational_authority: Account<'info, OperationalAuthority>,
    #[account(seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    #[account(mut,seeds=[LEDGER_SEED,virtual_pool.key().as_ref()],bump,has_one=virtual_pool)]
    pub ledger: Box<Account<'info, BuybackLedger>>,
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    /// CHECK: Zero-data system-owned custody PDA; signer only for fixed self CPI.
    #[account(mut,seeds=[VAULT_SEED,virtual_pool.key().as_ref()],bump,owner=anchor_lang::system_program::ID)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut,address=crate::treasury::ID)]
    pub treasury: SystemAccount<'info>,
    /// CHECK: Only this exact executable may receive the fixed discriminator.
    #[account(address=crate::ID,executable)]
    pub evolut_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_collect_buyback_fees<'info>(
    ctx: Context<'_, '_, '_, 'info, CollectBuybackFeesCtx<'info>>,
    kind: u8,
    expected_nonce: u64,
    max_claim_amount: u64,
    min_net_sol: u64,
    claim_nonce: [u8; 16],
) -> Result<()> {
    let a = &mut *ctx.accounts;
    let remaining = ctx.remaining_accounts;
    require!(
        a.policy.enabled && !a.policy.paused && kind < 2 && a.vault.data_is_empty(),
        PoolError::InvalidInput
    );
    let clock = Clock::get()?;
    let index = kind as usize;
    require!(
        a.ledger.last_collection_time[index] == 0
            || clock.unix_timestamp
                >= a.ledger.last_collection_time[index]
                    .checked_add(a.policy.cooldown_seconds as i64)
                    .ok_or(PoolError::MathOverflow)?,
        PoolError::NotPermitToDoThisAction
    );
    bump_nonce(&mut a.ledger.action_nonce, expected_nonce)?;
    require!(
        remaining.len() == if kind == 0 { 24 } else { 26 },
        PoolError::InvalidAccount
    );
    require_keys_eq!(remaining[0].key(), a.vault.key(), PoolError::InvalidAccount);
    require_keys_eq!(
        remaining[1].key(),
        a.virtual_pool.key(),
        PoolError::InvalidAccount
    );
    let pool = a.virtual_pool.load()?;
    require_keys_eq!(remaining[2].key(), pool.config, PoolError::InvalidAccount);
    require!(
        pool.is_migrated == 1
            && pool.base_mint == a.ledger.source_mint
            && pool.graduation.raydium_pool == a.ledger.raydium_pool
            && pool.graduation.partner_fee_nft == a.ledger.fee_nft,
        PoolError::InvalidAccount
    );
    require_keys_eq!(
        remaining[if kind == 0 { 9 } else { 7 }].key(),
        a.ledger.raydium_pool,
        PoolError::InvalidAccount
    );
    // Persistent creator ledger and ATAs must already exist, paid by enrollment's
    // keeper. The claim may spend/refund ONLY its two temporary-account rents.
    if kind == 1 {
        for i in [4usize, 15, 16, 17, 18] {
            require!(!remaining[i].data_is_empty(), PoolError::InvalidAccount);
        }
        require_keys_eq!(*remaining[4].owner, crate::ID, PoolError::InvalidAccount);
        for i in [15usize, 16, 17, 18] {
            require_keys_eq!(*remaining[i].owner, token::ID, PoolError::InvalidAccount);
        }
    }
    drop(pool);
    // Anyone can prefund a future system-owned temporary PDA. Its contribution
    // reduces our rent payment and is returned on close, but is a donation, not
    // Raydium revenue. Leave it as unallocated surplus instead of reserving it.
    let temporary_start = if kind == 0 { 17 } else { 23 };
    let temporary_prefunding = add(
        remaining[temporary_start].lamports(),
        remaining[temporary_start + 1].lamports(),
    )?;
    let before = a.vault.lamports();
    require!(
        before >= add(a.ledger.reserved()?, a.ledger.rent_reserve)?,
        PoolError::InvalidAccount
    );
    let advance = Rent::get()?
        .minimum_balance(165)
        .checked_mul(2)
        .ok_or(PoolError::MathOverflow)?;
    invoke_signed(
        &system_instruction::transfer(&a.operator.key(), &a.vault.key(), advance),
        &[
            a.operator.to_account_info(),
            a.vault.to_account_info(),
            a.system_program.to_account_info(),
        ],
        &[],
    )?;
    let pool_key = a.virtual_pool.key();
    let bump = [ctx.bumps.vault];
    let seeds: &[&[u8]] = &[VAULT_SEED, pool_key.as_ref(), &bump];
    let name = if kind == 0 {
        b"global:settle_buyback_lp_fees".as_slice()
    } else {
        b"global:settle_buyback_creator_fees".as_slice()
    };
    let mut data = anchor_lang::solana_program::hash::hash(name).to_bytes()[..8].to_vec();
    data.push(1);
    data.extend_from_slice(&min_net_sol.to_le_bytes());
    data.extend_from_slice(&claim_nonce);
    data.extend_from_slice(&max_claim_amount.to_le_bytes());
    data.extend_from_slice(&a.policy.max_impact_bps.to_le_bytes());
    data.extend_from_slice(&a.policy.max_slippage_bps.to_le_bytes());
    let accounts: Vec<_> = remaining
        .iter()
        .enumerate()
        .map(|(i, v)| (v, v.is_writable, i == 0))
        .collect();
    invoke_abi(&a.evolut_program, &accounts, data, &[seeds])?;
    // Pre-existing native donations remain outside every revenue/budget counter.
    // Both temporary rents returned in full; persistent initialization was forbidden.
    let net = sub(
        sub(sub(a.vault.lamports(), before)?, advance)?,
        temporary_prefunding,
    )?;
    require!(net >= min_net_sol, PoolError::ExceededSlippage);
    native_transfer(
        &a.vault.to_account_info(),
        &a.operator.to_account_info(),
        &a.system_program.to_account_info(),
        advance,
        seeds,
    )?;
    let treasury = a.ledger.allocate(net)?;
    native_transfer(
        &a.vault.to_account_info(),
        &a.treasury.to_account_info(),
        &a.system_program.to_account_info(),
        treasury,
        seeds,
    )?;
    a.ledger.last_collection_slot[index] = clock.slot;
    a.ledger.last_collection_time[index] = clock.unix_timestamp;
    require!(
        a.vault.lamports() >= add(a.ledger.reserved()?, a.ledger.rent_reserve)?,
        PoolError::InvalidAccount
    );
    emit!(BuybackRecorded {
        virtual_pool: pool_key,
        action: 0,
        nonce: expected_nonce,
        stage: a.ledger.last_stage,
        leg: kind,
        sol: net,
        tokens: 0,
        generation: a.ledger.generation
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CheckpointBuybackCtx<'info> {
    pub operator: Signer<'info>,
    #[account(seeds=[OperationalAuthority::SEED],bump,has_one=operator)]
    pub operational_authority: Account<'info, OperationalAuthority>,
    #[account(seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    #[account(mut,seeds=[LEDGER_SEED,virtual_pool.key().as_ref()],bump,has_one=virtual_pool)]
    pub ledger: Box<Account<'info, BuybackLedger>>,
    #[account(has_one=config)]
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    pub config: AccountLoader<'info, PoolConfig>,
}
pub fn handle_checkpoint_buyback(
    ctx: Context<CheckpointBuybackCtx>,
    stage: u8,
    event_hash: [u8; 32],
    milestone_slot: u64,
    expected_nonce: u64,
) -> Result<()> {
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    require!(
        a.policy.enabled
            && !a.policy.paused
            && a.ledger.is_evolving
            && stage >= 2
            && stage <= FINAL_STAGE
            && stage
                == a.ledger
                    .last_stage
                    .checked_add(1)
                    .ok_or(PoolError::MathOverflow)?
            && event_hash != [0; 32]
            && milestone_slot > 0
            && milestone_slot <= clock.slot,
        PoolError::InvalidInput
    );
    let kinds = if creator_fee_route(&a.config.load()?.raydium_amm_config) {
        2
    } else {
        1
    };
    for kind in 0..kinds {
        let observed = a.ledger.last_collection_slot[kind];
        require!(
            observed >= milestone_slot
                && observed <= clock.slot
                && clock.slot - observed <= COLLECTION_FRESH_SLOTS,
            PoolError::NotPermitToDoThisAction
        );
    }
    bump_nonce(&mut a.ledger.action_nonce, expected_nonce)?;
    let budget = a.ledger.pending;
    let generation = a.ledger.generation;
    let epoch = &mut a.ledger.epochs[(stage - 2) as usize];
    require!(epoch.stage == 0, PoolError::InvalidInput);
    epoch.stage = stage;
    epoch.event_hash = event_hash;
    epoch.milestone_slot = milestone_slot;
    epoch.checkpoint_slot = clock.slot;
    epoch.generation = generation;
    epoch.budget = budget;
    epoch.remaining = budget;
    for leg in 0..2 {
        a.ledger.frozen[leg] = add(a.ledger.frozen[leg], a.ledger.pending[leg])?;
        a.ledger.pending[leg] = 0;
    }
    a.ledger.last_stage = stage;
    emit!(BuybackRecorded {
        virtual_pool: a.virtual_pool.key(),
        action: 1,
        nonce: expected_nonce,
        stage,
        leg: 0,
        sol: add(budget[0], budget[1])?,
        tokens: 0,
        generation: a.ledger.generation
    });
    Ok(())
}

#[derive(Accounts)]
pub struct RecoverBuybackCtx<'info> {
    #[account(constraint=crate::admin::assert_eq_admin(admin.key()) @ PoolError::InvalidAdmin)]
    pub admin: Signer<'info>,
    #[account(seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    #[account(mut,seeds=[LEDGER_SEED,virtual_pool.key().as_ref()],bump,has_one=virtual_pool)]
    pub ledger: Box<Account<'info, BuybackLedger>>,
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    /// CHECK: Zero-data system custody PDA, fixed signer.
    #[account(mut,seeds=[VAULT_SEED,virtual_pool.key().as_ref()],bump,owner=anchor_lang::system_program::ID)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut,address=crate::treasury::ID)]
    pub treasury: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}
pub fn handle_recover_buyback(ctx: Context<RecoverBuybackCtx>, expected_nonce: u64) -> Result<()> {
    let a = &mut *ctx.accounts;
    require!(
        a.policy.paused && a.vault.data_is_empty(),
        PoolError::NotPermitToDoThisAction
    );
    bump_nonce(&mut a.ledger.action_nonce, expected_nonce)?;
    let amount = a.ledger.reserved()?;
    require!(
        a.vault.lamports() >= add(amount, a.ledger.rent_reserve)?,
        PoolError::InvalidAccount
    );
    let pool = a.virtual_pool.key();
    let bump = [ctx.bumps.vault];
    let seed: &[&[u8]] = &[VAULT_SEED, pool.as_ref(), &bump];
    native_transfer(
        &a.vault.to_account_info(),
        &a.treasury.to_account_info(),
        &a.system_program.to_account_info(),
        amount,
        seed,
    )?;
    a.ledger.pending = [0; 2];
    a.ledger.frozen = [0; 2];
    a.ledger.recovered = add128(a.ledger.recovered, amount as u128)?;
    for epoch in &mut a.ledger.epochs {
        epoch.remaining = [0; 2];
    }
    a.ledger.generation = add(a.ledger.generation, 1)?;
    emit!(BuybackRecorded {
        virtual_pool: pool,
        action: 3,
        nonce: expected_nonce,
        stage: a.ledger.last_stage,
        leg: 0,
        sol: amount,
        tokens: 0,
        generation: a.ledger.generation
    });
    Ok(())
}
fn native_transfer<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    amount: u64,
    seeds: &[&[u8]],
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    invoke_signed(
        &system_instruction::transfer(from.key, to.key, amount),
        &[from.clone(), to.clone(), system.clone()],
        &[seeds],
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct ExecuteBuybackCtx<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(seeds=[OperationalAuthority::SEED],bump,has_one=operator)]
    pub operational_authority: Account<'info, OperationalAuthority>,
    #[account(seeds=[POLICY_SEED],bump)]
    pub policy: Account<'info, BuybackPolicy>,
    #[account(mut,seeds=[LEDGER_SEED,virtual_pool.key().as_ref()],bump,has_one=virtual_pool)]
    pub ledger: Box<Account<'info, BuybackLedger>>,
    pub virtual_pool: AccountLoader<'info, VirtualPool>,
    /// CHECK: Fixed system-owned, empty custody signer.
    #[account(mut,seeds=[VAULT_SEED,virtual_pool.key().as_ref()],bump,owner=anchor_lang::system_program::ID)]
    pub vault: UncheckedAccount<'info>,
    /// CHECK: Bound to either enrolled source pool or one-time platform pool.
    #[account(mut)]
    pub raydium_pool: UncheckedAccount<'info>,
    /// CHECK: Official configuration and pool linkage checked below.
    pub raydium_amm_config: UncheckedAccount<'info>,
    /// CHECK: Only official CPMM executable, never a caller-selected swap route.
    #[account(address=CPMM_ID,executable)]
    pub cpmm_program: UncheckedAccount<'info>,
    /// CHECK: Official canonical PDA checked below.
    pub cpmm_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub base_mint: Box<Account<'info, Mint>>,
    #[account(address=token::spl_token::native_mint::ID)]
    pub quote_mint: Box<Account<'info, Mint>>,
    #[account(mut,token::mint=base_mint,token::authority=cpmm_authority)]
    pub base_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut,token::mint=quote_mint,token::authority=cpmm_authority)]
    pub quote_vault: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed,payer=operator,associated_token::mint=base_mint,associated_token::authority=vault)]
    pub destination: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed,payer=operator,associated_token::mint=quote_mint,associated_token::authority=vault)]
    pub source: Box<Account<'info, TokenAccount>>,
    /// CHECK: Canonical official pool observation account.
    #[account(mut,owner=CPMM_ID)]
    pub observation: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
pub fn handle_execute_buyback(
    ctx: Context<ExecuteBuybackCtx>,
    stage: u8,
    leg: u8,
    amount: u64,
    min_out: u64,
    deadline: i64,
    expected_nonce: u64,
) -> Result<()> {
    let a = &mut *ctx.accounts;
    let clock = Clock::get()?;
    require!(
        a.policy.enabled
            && !a.policy.paused
            && a.ledger.is_evolving
            && (2..=FINAL_STAGE).contains(&stage)
            && leg < 2
            && amount > 0
            && amount <= a.policy.max_batch_lamports
            && min_out > 0
            && deadline >= clock.unix_timestamp
            && deadline
                <= clock
                    .unix_timestamp
                    .checked_add(120)
                    .ok_or(PoolError::MathOverflow)?
            && a.vault.data_is_empty(),
        PoolError::InvalidInput
    );
    let index = leg as usize;
    let epoch_index = (stage - 2) as usize;
    let epoch = &a.ledger.epochs[epoch_index];
    require!(
        epoch.stage == stage
            && epoch.generation == a.ledger.generation
            && epoch.remaining[index] >= amount,
        PoolError::InvalidInput
    );
    require!(
        a.ledger.last_execution_time[index] == 0
            || clock.unix_timestamp
                >= a.ledger.last_execution_time[index]
                    .checked_add(a.policy.cooldown_seconds as i64)
                    .ok_or(PoolError::MathOverflow)?,
        PoolError::NotPermitToDoThisAction
    );
    let (target_mint, target_pool) = if leg == 0 {
        (a.ledger.source_mint, a.ledger.raydium_pool)
    } else {
        (a.policy.platform_mint, a.policy.platform_pool)
    };
    require!(
        target_mint != Pubkey::default() && target_mint != a.quote_mint.key(),
        PoolError::InvalidAccount
    );
    require_keys_eq!(a.base_mint.key(), target_mint, PoolError::InvalidAccount);
    require_keys_eq!(a.raydium_pool.key(), target_pool, PoolError::InvalidAccount);
    require!(
        a.base_mint.freeze_authority.is_none()
            && a.source.is_native.is_some()
            && a.destination.delegate.is_none()
            && a.source.delegate.is_none(),
        PoolError::InvalidAccount
    );
    let cp = decode_pool(&a.raydium_pool)?;
    let fee = decode_trading_config(&a.raydium_amm_config)?;
    require_keys_eq!(
        cp.amm_config,
        a.raydium_amm_config.key(),
        PoolError::InvalidAccount
    );
    if leg == 1 {
        require_keys_eq!(
            cp.amm_config,
            a.policy.platform_config,
            PoolError::InvalidAccount
        );
    }
    require!(
        cp.status & 4 == 0
            && clock.unix_timestamp >= 0
            && cp.open_time <= clock.unix_timestamp as u64
            && cp.token_0_program == token::ID
            && cp.token_1_program == token::ID,
        PoolError::InvalidAccount
    );
    let base_first = cp.token_0_mint == a.base_mint.key();
    require!(
        if base_first {
            cp.token_1_mint == a.quote_mint.key()
                && cp.token_0_vault == a.base_vault.key()
                && cp.token_1_vault == a.quote_vault.key()
        } else {
            cp.token_1_mint == a.base_mint.key()
                && cp.token_0_mint == a.quote_mint.key()
                && cp.token_1_vault == a.base_vault.key()
                && cp.token_0_vault == a.quote_vault.key()
        },
        PoolError::InvalidAccount
    );
    require_keys_eq!(
        cp.observation_key,
        a.observation.key(),
        PoolError::InvalidAccount
    );
    require_pda(
        &a.cpmm_authority,
        &CPMM_ID,
        &[b"vault_and_lp_mint_auth_seed"],
    )?;
    require_pda(
        &a.observation,
        &CPMM_ID,
        &[b"observation", a.raydium_pool.key().as_ref()],
    )?;
    require_pda(
        &a.base_vault.to_account_info(),
        &CPMM_ID,
        &[
            b"pool_vault",
            a.raydium_pool.key().as_ref(),
            a.base_mint.key().as_ref(),
        ],
    )?;
    require_pda(
        &a.quote_vault.to_account_info(),
        &CPMM_ID,
        &[
            b"pool_vault",
            a.raydium_pool.key().as_ref(),
            a.quote_mint.key().as_ref(),
        ],
    )?;
    let reserve = if base_first {
        effective_reserves(&cp, a.base_vault.amount, a.quote_vault.amount)?
    } else {
        effective_reserves(&cp, a.quote_vault.amount, a.base_vault.amount)?
    };
    let quote_index = if base_first { 1 } else { 0 };
    let floor = bounded_swap_floor(
        amount,
        reserve[quote_index],
        reserve[1 - quote_index],
        &cp,
        &fee,
        a.policy.max_impact_bps,
        a.policy.max_slippage_bps,
    )?;
    // Anyone can donate native SOL to a WSOL ATA. Normalize that unrelated
    // balance first, preserve it, and spend only the exact new reserved deposit.
    token::sync_native(CpiContext::new(
        a.token_program.to_account_info(),
        token::SyncNative {
            account: a.source.to_account_info(),
        },
    ))?;
    a.source.reload()?;
    let minimum = min_out.max(floor);
    let native_before = a.vault.lamports();
    let base_before = a.destination.amount;
    let quote_before = a.source.amount;
    let supply_before = a.base_mint.supply;
    require!(
        native_before >= add(a.ledger.reserved()?, a.ledger.rent_reserve)?,
        PoolError::InvalidAccount
    );
    bump_nonce(&mut a.ledger.action_nonce, expected_nonce)?;
    let pool_key = a.virtual_pool.key();
    let bump = [ctx.bumps.vault];
    let seed: &[&[u8]] = &[VAULT_SEED, pool_key.as_ref(), &bump];
    native_transfer(
        &a.vault.to_account_info(),
        &a.source.to_account_info(),
        &a.system_program.to_account_info(),
        amount,
        seed,
    )?;
    token::sync_native(CpiContext::new(
        a.token_program.to_account_info(),
        token::SyncNative {
            account: a.source.to_account_info(),
        },
    ))?;
    let infos = [
        a.vault.to_account_info(),
        a.cpmm_authority.to_account_info(),
        a.raydium_amm_config.to_account_info(),
        a.raydium_pool.to_account_info(),
        a.source.to_account_info(),
        a.destination.to_account_info(),
        a.quote_vault.to_account_info(),
        a.base_vault.to_account_info(),
        a.token_program.to_account_info(),
        a.token_program.to_account_info(),
        a.quote_mint.to_account_info(),
        a.base_mint.to_account_info(),
        a.observation.to_account_info(),
    ];
    let accounts: Vec<_> = infos
        .iter()
        .enumerate()
        .map(|(i, v)| (v, matches!(i, 3 | 4 | 5 | 6 | 7 | 12), i == 0))
        .collect();
    let mut data = vec![143, 190, 90, 218, 196, 30, 51, 222];
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&minimum.to_le_bytes());
    invoke_abi(&a.cpmm_program, &accounts, data, &[seed])?;
    a.source.reload()?;
    a.destination.reload()?;
    require!(a.source.amount == quote_before, PoolError::InvalidAccount);
    let bought = sub(a.destination.amount, base_before)?;
    require!(bought >= minimum, PoolError::ExceededSlippage);
    let burn = token::spl_token::instruction::burn_checked(
        &token::ID,
        &a.destination.key(),
        &a.base_mint.key(),
        &a.vault.key(),
        &[],
        bought,
        a.base_mint.decimals,
    )?;
    invoke_signed(
        &burn,
        &[
            a.destination.to_account_info(),
            a.base_mint.to_account_info(),
            a.vault.to_account_info(),
            a.token_program.to_account_info(),
        ],
        &[seed],
    )?;
    a.destination.reload()?;
    a.base_mint.reload()?;
    require!(
        a.destination.amount == base_before
            && sub(supply_before, a.base_mint.supply)? == bought
            && sub(native_before, a.vault.lamports())? == amount,
        PoolError::InvalidAccount
    );
    a.ledger.epochs[epoch_index].remaining[index] =
        sub(a.ledger.epochs[epoch_index].remaining[index], amount)?;
    a.ledger.epochs[epoch_index].burned[index] =
        add128(a.ledger.epochs[epoch_index].burned[index], bought as u128)?;
    a.ledger.frozen[index] = sub(a.ledger.frozen[index], amount)?;
    a.ledger.spent[index] = add128(a.ledger.spent[index], amount as u128)?;
    a.ledger.burned[index] = add128(a.ledger.burned[index], bought as u128)?;
    a.ledger.last_execution_time[index] = clock.unix_timestamp;
    require!(
        a.vault.lamports() >= add(a.ledger.reserved()?, a.ledger.rent_reserve)?,
        PoolError::InvalidAccount
    );
    emit!(BuybackRecorded {
        virtual_pool: pool_key,
        action: 2,
        nonce: expected_nonce,
        stage,
        leg,
        sol: amount,
        tokens: bought,
        generation: a.ledger.generation
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifetime_dust_split_never_overdraws_current_receipt() {
        let mut ledger = BuybackLedger {
            is_evolving: true,
            last_stage: 1,
            ..BuybackLedger::default()
        };
        let mut treasury = 0;
        for _ in 0..101 {
            treasury += ledger.allocate(1).unwrap();
        }
        assert_eq!(ledger.pending, [10, 10]);
        assert_eq!(treasury, 81);
        assert_eq!(ledger.reserved().unwrap() + treasury, 101);
        assert_eq!(ledger.total_net_sol, 101);
        assert_eq!(ledger.allocated_total, 20);
    }
    #[test]
    fn partitioned_receipts_keep_identical_reservations() {
        for n in 1..500 {
            let mut once = BuybackLedger {
                is_evolving: true,
                last_stage: 1,
                ..BuybackLedger::default()
            };
            let mut split = BuybackLedger {
                is_evolving: true,
                last_stage: 1,
                ..BuybackLedger::default()
            };
            once.allocate(n).unwrap();
            for _ in 0..n {
                split.allocate(1).unwrap();
            }
            assert_eq!(once.pending, split.pending);
            assert_eq!(once.treasury_paid, split.treasury_paid);
            assert!((split.pending[0] as i64 - split.pending[1] as i64).abs() <= 1);
        }
    }
    #[test]
    fn classic_and_post_final_receipts_leave_previous_reserves_untouched() {
        let mut classic = BuybackLedger {
            last_stage: 1,
            ..BuybackLedger::default()
        };
        assert_eq!(classic.allocate(999).unwrap(), 999);
        assert_eq!(classic.reserved().unwrap(), 0);
        let mut evolving = BuybackLedger {
            is_evolving: true,
            last_stage: 7,
            ..BuybackLedger::default()
        };
        assert_eq!(evolving.allocate(100).unwrap(), 80);
        evolving.last_stage = 8;
        assert_eq!(evolving.allocate(100).unwrap(), 100);
        assert_eq!(evolving.pending, [10, 10]);
        assert_eq!(evolving.eligible_net_sol, 100);
    }
    #[test]
    fn stale_nonce_cannot_repeat_an_action() {
        let mut n = 4;
        bump_nonce(&mut n, 4).unwrap();
        assert_eq!(n, 5);
        assert!(bump_nonce(&mut n, 4).is_err());
        assert_eq!(n, 5);
    }
}
