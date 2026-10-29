use crate::{
    const_pda,
    params::liquidity_distribution::get_migration_base_token,
    raydium::*,
    safe_math::SafeMath,
    state::{GraduationReceipt, MigrationProgress, PoolConfig, VirtualPool},
    PoolError, PoolGraduated,
};
use anchor_lang::{
    prelude::*,
    solana_program::{program::invoke, system_instruction},
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount},
};

/// Account order is part of the generated Anchor ABI. Future accounts are unchecked only
/// because the official CPIs create them; every address and initial owner is checked below.
#[event_cpi]
#[derive(Accounts)]
pub struct GraduateCtx<'info> {
    #[account(mut)]
    pub payer: Signer<'info>, // 0
    #[account(mut, address = const_pda::pool_authority::ID)]
    pub pool_authority: SystemAccount<'info>, // 1
    #[account(has_one = quote_mint, has_one = raydium_amm_config)]
    pub config: AccountLoader<'info, PoolConfig>, // 2
    #[account(mut, has_one = config, has_one = base_mint, has_one = base_vault, has_one = quote_vault)]
    pub virtual_pool: AccountLoader<'info, VirtualPool>, // 3
    #[account(mut)]
    pub base_mint: Box<Account<'info, Mint>>, // 4
    #[account(address = token::spl_token::native_mint::ID)]
    pub quote_mint: Box<Account<'info, Mint>>, // 5
    #[account(mut, token::mint = base_mint, token::authority = pool_authority)]
    pub base_vault: Box<Account<'info, TokenAccount>>, // 6
    #[account(mut, token::mint = quote_mint, token::authority = pool_authority)]
    pub quote_vault: Box<Account<'info, TokenAccount>>, // 7
    /// An existing caller-owned WSOL ATA. Wallet preparation is not pool initialization.
    #[account(mut, associated_token::mint = quote_mint, associated_token::authority = payer)]
    pub reimbursement: Box<Account<'info, TokenAccount>>, // 8
    /// CHECK: official executable.
    #[account(address = CPMM_ID, executable)]
    pub cpmm_program: UncheckedAccount<'info>, // 9
    /// CHECK: official executable.
    #[account(address = LOCK_ID, executable)]
    pub lock_program: UncheckedAccount<'info>, // 10
    /// CHECK: typed ABI decode and policy comparison.
    pub raydium_amm_config: UncheckedAccount<'info>, // 11
    /// CHECK: official authority PDA.
    pub cpmm_authority: UncheckedAccount<'info>, // 12
    /// CHECK: Evolut PDA containing pool + nonce, fresh system account.
    #[account(mut)]
    pub raydium_pool: UncheckedAccount<'info>, // 13
    /// CHECK: official LP mint PDA, fresh.
    #[account(mut)]
    pub lp_mint: UncheckedAccount<'info>, // 14
    /// CHECK: pool authority LP ATA, fresh.
    #[account(mut)]
    pub source_lp: UncheckedAccount<'info>, // 15
    /// CHECK: official vault PDA, fresh.
    #[account(mut)]
    pub token_0_vault: UncheckedAccount<'info>, // 16
    /// CHECK: official vault PDA, fresh.
    #[account(mut)]
    pub token_1_vault: UncheckedAccount<'info>, // 17
    /// CHECK: fixed official WSOL creation-fee account.
    #[account(mut, address = CREATE_FEE_RECEIVER)]
    pub create_pool_fee: UncheckedAccount<'info>, // 18
    /// CHECK: official observation PDA, fresh.
    #[account(mut)]
    pub observation: UncheckedAccount<'info>, // 19
    pub token_program: Program<'info, Token>, // 20
    pub associated_token_program: Program<'info, AssociatedToken>, // 21
    pub system_program: Program<'info, System>, // 22
    pub rent: Sysvar<'info, Rent>,            // 23
    /// CHECK: official lock authority PDA.
    pub lock_authority: UncheckedAccount<'info>, // 24
    /// CHECK: must equal the current actual curve creator.
    pub creator: UncheckedAccount<'info>, // 25
    /// CHECK: Evolut nonce-bound NFT mint PDA, fresh.
    #[account(mut)]
    pub creator_fee_nft: UncheckedAccount<'info>, // 26
    /// CHECK: creator's NFT ATA, fresh.
    #[account(mut)]
    pub creator_nft_account: UncheckedAccount<'info>, // 27
    /// CHECK: official lock-state PDA, fresh.
    #[account(mut)]
    pub creator_lock: UncheckedAccount<'info>, // 28
    /// CHECK: official lock authority LP ATA, fresh, shared by both locks.
    #[account(mut)]
    pub locked_lp_vault: UncheckedAccount<'info>, // 29
    /// CHECK: canonical metadata PDA; no metadata is created.
    #[account(mut)]
    pub creator_metadata: UncheckedAccount<'info>, // 30
    /// CHECK: immutable canonical Evolut beneficiary.
    #[account(address = EVOLUT_AUTHORITY)]
    pub partner: UncheckedAccount<'info>, // 31
    /// CHECK: Evolut nonce-bound NFT mint PDA, fresh.
    #[account(mut)]
    pub partner_fee_nft: UncheckedAccount<'info>, // 32
    /// CHECK: partner's NFT ATA, fresh.
    #[account(mut)]
    pub partner_nft_account: UncheckedAccount<'info>, // 33
    /// CHECK: official lock-state PDA, fresh.
    #[account(mut)]
    pub partner_lock: UncheckedAccount<'info>, // 34
    /// CHECK: canonical metadata PDA; no metadata is created.
    #[account(mut)]
    pub partner_metadata: UncheckedAccount<'info>, // 35
    /// CHECK: official executable, required by locker even with metadata disabled.
    #[account(address = METADATA_ID, executable)]
    pub metadata_program: UncheckedAccount<'info>, // 36
}

pub fn handle_graduate<'info>(ctx: Context<'_, '_, '_, 'info, GraduateCtx<'info>>, nonce: [u8; 32]) -> Result<()> {
    let config = ctx.accounts.config.load()?;
    let mut pool = ctx.accounts.virtual_pool.load_mut()?;
    require!(
        pool.is_migrated == 0
            && pool.graduation.raydium_pool == Pubkey::default()
            && pool.get_migration_progress()? == MigrationProgress::LockedVesting,
        PoolError::NotPermitToDoThisAction
    );
    require!(
        pool.pool_type == 0
            && config.token_type == 0
            && matches!(config.version, 1 | 2)
            && config.creator_trading_fee_percentage == 50,
        PoolError::InvalidInput
    );
    require_keys_eq!(
        pool.creator,
        ctx.accounts.creator.key(),
        PoolError::InvalidAccount
    );
    require_keys_eq!(
        config.fee_claimer,
        ctx.accounts.partner.key(),
        PoolError::InvalidAccount
    );
    require!(
        ctx.accounts.pool_authority.data_is_empty(),
        PoolError::InvalidAccount
    );
    require!(
        ctx.accounts.reimbursement.is_native.is_some()
            && ctx.accounts.quote_vault.is_native.is_some(),
        PoolError::InvalidQuoteMint
    );
    let ray = validate_fee_config(&ctx.accounts.raydium_amm_config)?;
    require!(
        ray.trade_fee_rate == config.raydium_trade_fee_rate
            && ray.protocol_fee_rate == config.raydium_protocol_fee_rate
            && ray.fund_fee_rate == config.raydium_fund_fee_rate
            && ray.create_pool_fee == config.raydium_create_pool_fee,
        PoolError::InvalidInput
    );
    let creator_route = creator_fee_route(&ctx.accounts.raydium_amm_config.key());
    let mut a = graduation_infos(ctx.accounts);
    // Keep the existing 39-account ABI intact. Only #13 appends the per-pool
    // collection authority and the permission for the authorized payer PDA.
    require!(ctx.remaining_accounts.len() == if creator_route { 2 } else { 0 }, PoolError::InvalidAccount);
    if creator_route {
        require_pda(&ctx.remaining_accounts[0], &crate::ID,
            &[CREATOR_AUTH_SEED, ctx.accounts.virtual_pool.key().as_ref()])?;
        validate_permission(&ctx.remaining_accounts[1], &ctx.accounts.pool_authority.key())?;
        a.extend_from_slice(ctx.remaining_accounts);
    }
    let base_is_zero = ctx.accounts.base_mint.key() < ctx.accounts.quote_mint.key();
    let (mint0, mint1, source0, source1) = if base_is_zero {
        (4, 5, 6, 7)
    } else {
        (5, 4, 7, 6)
    };
    let pool_key = ctx.accounts.virtual_pool.key();
    let (destination, pool_bump) =
        Pubkey::find_program_address(&[POOL_SEED, pool_key.as_ref(), &nonce], &crate::ID);
    require_keys_eq!(destination, a[13].key(), PoolError::InvalidAccount);
    require_pda(&a[12], &CPMM_ID, &[b"vault_and_lp_mint_auth_seed"])?;
    require_pda(&a[14], &CPMM_ID, &[b"pool_lp_mint", destination.as_ref()])?;
    require_pda(
        &a[16],
        &CPMM_ID,
        &[b"pool_vault", destination.as_ref(), a[mint0].key.as_ref()],
    )?;
    require_pda(
        &a[17],
        &CPMM_ID,
        &[b"pool_vault", destination.as_ref(), a[mint1].key.as_ref()],
    )?;
    require_pda(&a[19], &CPMM_ID, &[b"observation", destination.as_ref()])?;
    require_pda(&a[24], &LOCK_ID, &[b"lock_cp_authority_seed"])?;
    require_ata(&a[15], a[1].key, a[14].key)?;
    require_ata(&a[29], a[24].key, a[14].key)?;
    let mut nft_bumps = [0; 2];
    for (position, owner, mint, ata, lock, metadata) in
        [(0, 25, 26, 27, 28, 30), (1, 31, 32, 33, 34, 35)]
    {
        let (nft, bump) = Pubkey::find_program_address(
            &[NFT_SEED, pool_key.as_ref(), &nonce, &[position]],
            &crate::ID,
        );
        nft_bumps[position as usize] = bump;
        require_keys_eq!(nft, a[mint].key(), PoolError::InvalidAccount);
        require_ata(&a[ata], a[owner].key, a[mint].key)?;
        require_pda(&a[lock], &LOCK_ID, &[b"locked_liquidity", nft.as_ref()])?;
        require_pda(
            &a[metadata],
            &METADATA_ID,
            &[b"metadata", METADATA_ID.as_ref(), nft.as_ref()],
        )?;
        require!(
            a[metadata].data_is_empty() && *a[metadata].owner == anchor_lang::system_program::ID,
            PoolError::InvalidAccount
        );
    }
    let rent = Rent::get()?;
    let mut forecast = ray.create_pool_fee;
    for (index, size) in [
        (13, 637),
        (14, 82),
        (15, 165),
        (16, 165),
        (17, 165),
        (19, 4075),
        (26, 82),
        (27, 165),
        (28, 256),
        (29, 165),
        (32, 82),
        (33, 165),
        (34, 256),
    ] {
        forecast = forecast.safe_add(creation_shortfall(&a[index], size, &rent)?)?;
    }
    require!(
        forecast <= config.max_setup_lamports && forecast < config.migration_quote_threshold,
        PoolError::InvalidInput
    );
    let graduation_fee = config.graduation_fee_lamports()?;
    let quote_deposit = config.graduation_quote_deposit(forecast)?;
    let base_deposit = get_migration_base_token(quote_deposit, config.migration_sqrt_price)?;
    require!(
        base_deposit > 0 && base_deposit <= config.migration_base_threshold,
        PoolError::InvalidInput
    );
    let base_fees = pool.get_protocol_and_trading_base_fee()?;
    let quote_fees = pool
        .protocol_quote_fee
        .safe_add(pool.partner_quote_fee)?
        .safe_add(pool.creator_quote_fee)?;
    let surplus = pool.get_total_surplus(config.migration_quote_threshold)?;
    let mut protected_quote = quote_fees;
    if pool.is_partner_withdraw_surplus == 0 {
        protected_quote = protected_quote.safe_add(pool.get_partner_surplus(&config, surplus)?)?;
    }
    if pool.is_creator_withdraw_surplus == 0 {
        protected_quote = protected_quote.safe_add(pool.get_creator_surplus(&config, surplus)?)?;
    }
    if pool.is_protocol_withdraw_surplus == 0 {
        protected_quote = protected_quote
            .safe_add(pool.get_protocol_surplus(config.migration_quote_threshold)?)?;
    }
    let base_before = ctx.accounts.base_vault.amount;
    let quote_before = ctx.accounts.quote_vault.amount;
    require!(
        base_before >= config.migration_base_threshold.safe_add(base_fees)?
            && quote_before >= config.migration_quote_threshold.safe_add(protected_quote)?,
        PoolError::InsufficientLiquidityForMigration
    );
    let authority_before = a[1].lamports();
    invoke(
        &system_instruction::transfer(a[0].key, a[1].key, forecast),
        &[a[0].clone(), a[1].clone(), a[22].clone()],
    )?;
    let auth_bump = [const_pda::pool_authority::BUMP];
    let auth_seed: &[&[u8]] = &[crate::constants::seeds::POOL_AUTHORITY_PREFIX, &auth_bump];
    let pool_bump = [pool_bump];
    let destination_seed: &[&[u8]] = &[POOL_SEED, pool_key.as_ref(), &nonce, &pool_bump];
    let nft0_bump = [nft_bumps[0]];
    let nft1_bump = [nft_bumps[1]];
    let nft0_seed: &[&[u8]] = &[NFT_SEED, pool_key.as_ref(), &nonce, &[0], &nft0_bump];
    let nft1_seed: &[&[u8]] = &[NFT_SEED, pool_key.as_ref(), &nonce, &[1], &nft1_bump];
    let seeds = &[auth_seed, destination_seed, nft0_seed, nft1_seed];
    let (amount0, amount1) = if base_is_zero {
        (base_deposit, quote_deposit)
    } else {
        (quote_deposit, base_deposit)
    };
    let cp_order = vec![
        (1, true, true),
        (11, false, false),
        (12, false, false),
        (13, true, true),
        (mint0, false, false),
        (mint1, false, false),
        (14, true, false),
        (source0, true, false),
        (source1, true, false),
        (15, true, false),
        (16, true, false),
        (17, true, false),
        (18, true, false),
        (19, true, false),
        (20, false, false),
        (20, false, false),
        (20, false, false),
        (21, false, false),
        (22, false, false),
        (23, false, false),
    ];
    if creator_route {
        // Permissioned ABI adds creator after payer, permission after observation,
        // and omits Rent. The authorized pool_authority still pays/signs.
        let mut permission_order = cp_order.clone();
        permission_order.pop();
        permission_order.insert(1, (39, false, false));
        permission_order.insert(15, (40, false, false));
        invoke_order(&a, 9, &permission_order, initialize_permission_data(amount0, amount1), seeds)?;
    } else {
        invoke_order(&a, 9, &cp_order, initialize_data(amount0, amount1), seeds)?;
    }
    let source_lp = read_token(&a[15])?;
    require_keys_eq!(source_lp.owner, a[1].key(), PoolError::InvalidAccount);
    require_keys_eq!(source_lp.mint, a[14].key(), PoolError::InvalidAccount);
    let lp_amount = source_lp.amount;
    require!(lp_amount >= 2, PoolError::InvalidInput);
    let creator_lp = lp_amount / 2;
    let partner_lp = lp_amount.safe_sub(creator_lp)?;
    for (owner, mint, ata, lock, metadata, amount) in [
        (25, 26, 27, 28, 30, creator_lp),
        (31, 32, 33, 34, 35, partner_lp),
    ] {
        let order = vec![
            (24, false, false),
            (1, true, true),
            (1, false, true),
            (owner, false, false),
            (mint, true, true),
            (ata, true, false),
            (13, false, false),
            (lock, true, false),
            (14, false, false),
            (15, true, false),
            (29, true, false),
            (16, true, false),
            (17, true, false),
            (metadata, true, false),
            (23, false, false),
            (22, false, false),
            (20, false, false),
            (21, false, false),
            (36, false, false),
        ];
        invoke_order(&a, 10, &order, lock_data(amount), seeds)?;
        verify_lock(&a, owner, mint, ata, lock, amount)?;
    }
    let measured = authority_before
        .safe_add(forecast)?
        .safe_sub(a[1].lamports())?;
    require!(
        measured == forecast
            && a[1].data_is_empty()
            && *a[1].owner == anchor_lang::system_program::ID,
        PoolError::InvalidAccount
    );
    require!(read_token(&a[15])?.amount == 0, PoolError::InvalidAccount);
    let locked_lp = read_token(&a[29])?;
    require!(locked_lp.amount == lp_amount, PoolError::InvalidAccount);
    require_keys_eq!(locked_lp.owner, a[24].key(), PoolError::InvalidAccount);
    require_keys_eq!(locked_lp.mint, a[14].key(), PoolError::InvalidAccount);
    verify_pool(&a, mint0, mint1, lp_amount, amount0, amount1)?;
    // Reimburse measured native setup expense as WSOL, not caller-selected SOL destinations.
    token::transfer(
        CpiContext::new_with_signer(
            a[20].clone(),
            token::Transfer {
                from: a[7].clone(),
                to: a[8].clone(),
                authority: a[1].clone(),
            },
            &[auth_seed],
        ),
        measured,
    )?;
    ctx.accounts.base_vault.reload()?;
    ctx.accounts.quote_vault.reload()?;
    require!(
        ctx.accounts.base_vault.amount == base_before.safe_sub(base_deposit)?
            && ctx.accounts.quote_vault.amount
                == quote_before.safe_sub(config.migration_quote_threshold)?.safe_add(graduation_fee)?
            && ctx.accounts.quote_vault.amount >= protected_quote.safe_add(graduation_fee)?,
        PoolError::InvalidAccount
    );
    // The ledger excludes trading fees but still includes creator vesting already moved
    // by create_locker. Never treat unsolicited vault donations as issued curve supply.
    let vested = config
        .locked_vesting_config
        .to_locked_vesting_params()
        .get_total_amount()?;
    let (burnable, claimable_leftover) = config.graduation_burn_amounts(
        pool.base_reserve, vested, base_deposit, ctx.accounts.base_vault.amount, base_fees,
    )?;
    if burnable > 0 {
        let supply_before_burn = ctx.accounts.base_mint.supply;
        let vault_before_burn = ctx.accounts.base_vault.amount;
        token::burn(
            CpiContext::new_with_signer(
                a[20].clone(),
                token::Burn {
                    mint: a[4].clone(),
                    from: a[6].clone(),
                    authority: a[1].clone(),
                },
                &[auth_seed],
            ),
            burnable,
        )?;
        ctx.accounts.base_mint.reload()?;
        ctx.accounts.base_vault.reload()?;
        require!(
            ctx.accounts.base_mint.supply == supply_before_burn.safe_sub(burnable)?
                && ctx.accounts.base_vault.amount == vault_before_burn.safe_sub(burnable)?,
            PoolError::InvalidAccount
        );
    }
    let slot = Clock::get()?.slot;
    // Fixed revenue stays as WSOL in the source vault, claimable only through
    // the existing treasury-authorized partner-fee instruction. It is NOT a
    // worker reimbursement or a 50/50 trading fee. Atomic receipt guard above
    // prevents charging again on replay; any failed CPI rolls the credit back.
    pool.partner_quote_fee = pool.partner_quote_fee.safe_add(graduation_fee)?;
    pool.leftover_amount = claimable_leftover;
    pool.graduation = GraduationReceipt {
        raydium_pool: destination,
        lp_mint: a[14].key(),
        creator_fee_nft: a[26].key(),
        partner_fee_nft: a[32].key(),
        creator_lock: a[28].key(),
        partner_lock: a[34].key(),
        initial_creator: a[25].key(),
        initial_partner: a[31].key(),
        nonce,
        creator_lp_amount: creator_lp,
        partner_lp_amount: partner_lp,
        setup_lamports: measured,
        base_deposited: base_deposit,
        quote_deposited: quote_deposit,
        slot,
    };
    // Preserve historical quote_reserve: surplus claims depend on it after graduation.
    pool.update_after_create_pool();
    pool.set_migration_progress(MigrationProgress::CreatedPool.into());
    drop(pool);
    drop(config);
    emit_cpi!(PoolGraduated {
        virtual_pool: pool_key,
        config: a[2].key(),
        raydium_amm_config: a[11].key(),
        raydium_pool: destination,
        lp_mint: a[14].key(),
        base_mint: a[4].key(),
        quote_mint: a[5].key(),
        token_0_vault: a[16].key(),
        token_1_vault: a[17].key(),
        creator_fee_nft: a[26].key(),
        partner_fee_nft: a[32].key(),
        creator_lock: a[28].key(),
        partner_lock: a[34].key(),
        initial_creator: a[25].key(),
        initial_partner: a[31].key(),
        nonce,
        creator_lp_amount: creator_lp,
        partner_lp_amount: partner_lp,
        setup_lamports: measured,
        base_deposited: base_deposit,
        quote_deposited: quote_deposit,
        slot,
    });
    Ok(())
}

fn invoke_order<'info>(
    accounts: &[AccountInfo<'info>],
    program: usize,
    order: &[(usize, bool, bool)],
    data: Vec<u8>,
    seeds: &[&[&[u8]]],
) -> Result<()> {
    let ordered: Vec<_> = order
        .iter()
        .map(|(i, w, s)| (&accounts[*i], *w, *s))
        .collect();
    invoke_abi(&accounts[program], &ordered, data, seeds)
}
#[inline(never)]
fn read_token(account: &AccountInfo) -> Result<Box<TokenAccount>> {
    require_keys_eq!(*account.owner, token::ID, PoolError::InvalidAccount);
    Ok(Box::new(TokenAccount::try_deserialize(
        &mut account.try_borrow_data()?.as_ref(),
    )?))
}

// Avoid Anchor's per-field Vec allocations on the fixed 32KiB SBF heap.
#[inline(never)]
fn graduation_infos<'info>(accounts: &GraduateCtx<'info>) -> Vec<AccountInfo<'info>> {
    vec![
        accounts.payer.to_account_info(),
        accounts.pool_authority.to_account_info(),
        accounts.config.to_account_info(),
        accounts.virtual_pool.to_account_info(),
        accounts.base_mint.to_account_info(),
        accounts.quote_mint.to_account_info(),
        accounts.base_vault.to_account_info(),
        accounts.quote_vault.to_account_info(),
        accounts.reimbursement.to_account_info(),
        accounts.cpmm_program.to_account_info(),
        accounts.lock_program.to_account_info(),
        accounts.raydium_amm_config.to_account_info(),
        accounts.cpmm_authority.to_account_info(),
        accounts.raydium_pool.to_account_info(),
        accounts.lp_mint.to_account_info(),
        accounts.source_lp.to_account_info(),
        accounts.token_0_vault.to_account_info(),
        accounts.token_1_vault.to_account_info(),
        accounts.create_pool_fee.to_account_info(),
        accounts.observation.to_account_info(),
        accounts.token_program.to_account_info(),
        accounts.associated_token_program.to_account_info(),
        accounts.system_program.to_account_info(),
        accounts.rent.to_account_info(),
        accounts.lock_authority.to_account_info(),
        accounts.creator.to_account_info(),
        accounts.creator_fee_nft.to_account_info(),
        accounts.creator_nft_account.to_account_info(),
        accounts.creator_lock.to_account_info(),
        accounts.locked_lp_vault.to_account_info(),
        accounts.creator_metadata.to_account_info(),
        accounts.partner.to_account_info(),
        accounts.partner_fee_nft.to_account_info(),
        accounts.partner_nft_account.to_account_info(),
        accounts.partner_lock.to_account_info(),
        accounts.partner_metadata.to_account_info(),
        accounts.metadata_program.to_account_info(),
        accounts.event_authority.to_account_info(),
        accounts.program.to_account_info(),
    ]
}

#[inline(never)]
fn verify_lock(
    a: &[AccountInfo],
    owner: usize,
    mint: usize,
    ata: usize,
    lock: usize,
    amount: u64,
) -> Result<()> {
    let state = decode_lock(&a[lock])?;
    require!(
        state.locked_lp_amount == amount
            && state.claimed_lp_amount == 0
            && state.unclaimed_lp_amount == 0,
        PoolError::InvalidAccount
    );
    require_keys_eq!(state.pool_id, a[13].key(), PoolError::InvalidAccount);
    require_keys_eq!(state.fee_nft_mint, a[mint].key(), PoolError::InvalidAccount);
    require_keys_eq!(state.lp_mint, a[14].key(), PoolError::InvalidAccount);
    let nft_account = read_token(&a[ata])?;
    require!(nft_account.amount == 1, PoolError::InvalidAccount);
    require_keys_eq!(nft_account.owner, a[owner].key(), PoolError::InvalidAccount);
    require_keys_eq!(nft_account.mint, a[mint].key(), PoolError::InvalidAccount);
    Ok(())
}
#[inline(never)]
fn verify_pool(
    a: &[AccountInfo],
    mint0: usize,
    mint1: usize,
    lp_amount: u64,
    amount0: u64,
    amount1: u64,
) -> Result<()> {
    let cp = decode_pool(&a[13])?;
    require_keys_eq!(cp.amm_config, a[11].key(), PoolError::InvalidAccount);
    let creator_route = creator_fee_route(a[11].key);
    require_keys_eq!(cp.pool_creator, a[if creator_route { 39 } else { 1 }].key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.lp_mint, a[14].key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.token_0_mint, a[mint0].key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.token_1_mint, a[mint1].key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.token_0_vault, a[16].key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.token_1_vault, a[17].key(), PoolError::InvalidAccount);
    require_keys_eq!(cp.observation_key, a[19].key(), PoolError::InvalidAccount);
    require!(
        cp.status == 0
            && cp.enable_creator_fee == creator_route
            && cp.creator_fee_on == 0
            && cp.token_0_program == token::ID
            && cp.token_1_program == token::ID
            && cp.lp_supply == lp_amount.safe_add(100)?
            && read_token(&a[16])?.amount == amount0
            && read_token(&a[17])?.amount == amount1,
        PoolError::InvalidAccount
    );

    Ok(())
}
