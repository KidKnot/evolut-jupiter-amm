//! Jupiter adapter for the reviewed Evolut mainnet curve ABI.
//!
//! This initial adapter supports legacy SPL Token / WSOL pools with flat 1% or
//! 2% fees collected in WSOL. Token-2022, dynamic/scheduled fees, referrals,
//! ExactOut, and partial fills are deliberately rejected. `Swap::Placeholder`
//! is the integration-kit native-instruction seam, not a deployed Jupiter router
//! variant. The caller must already have funded, initialized token accounts.

mod quote;

use anchor_lang::Discriminator;
use dynamic_bonding_curve::state::{PoolConfig, VirtualPool};
use jupiter_amm_interface::{
    AccountProvider, Amm, AmmContext, AmmError, AmmLabel, ClockRef, KeyedAccount, Quote,
    QuoteParams, SingleProgramAmm, Swap, SwapAndAccountMetas, SwapMode, SwapParams,
};
use solana_account::ReadableAccount;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::{Pubkey, pubkey};
use std::{collections::HashSet, sync::atomic::Ordering};

pub const PROGRAM_ID: Pubkey = pubkey!("EVoLutQjaoK3fkQh1ULbQzqVruU4yYEdSsqZB5djTTQZ");
pub const WSOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
pub const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const POOL_ACCOUNT_LEN: usize = 760;
pub const CONFIG_ACCOUNT_LEN: usize = 1048;
pub const SWAP_DISCRIMINATOR: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];

type Result<T> = std::result::Result<T, AmmError>;

#[derive(Clone)]
pub struct EvolutAmm {
    key: Pubkey,
    pool: VirtualPool,
    config: Option<PoolConfig>,
    clock: ClockRef,
    base_vault_amount: u64,
    quote_vault_amount: u64,
    ready: bool,
}

impl SingleProgramAmm for EvolutAmm {
    const PROGRAM_ID: Pubkey = PROGRAM_ID;
    const LABEL: AmmLabel = "Evolut";
}

/// Encode the existing native `swap` instruction: exactly 24 bytes, ExactIn.
/// The second amount is the caller's minimum acceptable output, in raw units.
pub fn encode_native_swap(amount_in: u64, minimum_amount_out: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(24);
    data.extend_from_slice(&SWAP_DISCRIMINATOR);
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&minimum_amount_out.to_le_bytes());
    data
}

pub fn pool_authority() -> Pubkey {
    derive(PROGRAM_ID, &[b"pool_authority"])
}

pub fn event_authority() -> Pubkey {
    derive(PROGRAM_ID, &[b"__event_authority"])
}

pub fn associated_token_address(owner: Pubkey, mint: Pubkey) -> Pubkey {
    derive(
        ASSOCIATED_TOKEN_PROGRAM_ID,
        &[owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
    )
}

fn derive(program: Pubkey, seeds: &[&[u8]]) -> Pubkey {
    // The deployed Anchor contract and Jupiter use different Solana crate
    // generations. Convert keys through their stable 32-byte representation.
    let program = anchor_lang::prelude::Pubkey::new_from_array(program.to_bytes());
    from_contract(anchor_lang::prelude::Pubkey::find_program_address(seeds, &program).0)
}

fn from_contract(key: anchor_lang::prelude::Pubkey) -> Pubkey {
    Pubkey::new_from_array(key.to_bytes())
}

fn check(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(AmmError::from(message))
    }
}

fn decode<T: bytemuck::Pod + Discriminator>(
    account: &impl ReadableAccount,
    len: usize,
) -> Result<T> {
    check(
        account.owner() == &PROGRAM_ID,
        "incorrect Evolut account owner",
    )?;
    check(!account.executable(), "state account cannot be executable")?;
    let data = account.data();
    check(data.len() == len, "unsupported Evolut account length")?;
    check(
        data.starts_with(T::DISCRIMINATOR),
        "incorrect Evolut account discriminator",
    )?;
    bytemuck::try_pod_read_unaligned(&data[8..])
        .map_err(|e| AmmError::from(format!("invalid Evolut layout: {e}")))
}

fn validate_pool(key: Pubkey, pool: &VirtualPool) -> Result<()> {
    let config = from_contract(pool.config);
    let base = from_contract(pool.base_mint);
    check(
        config != Pubkey::default() && base != Pubkey::default() && base != WSOL_MINT,
        "invalid pool mint/config",
    )?;
    check(pool.pool_type == 0, "Token-2022 pools are unsupported")?;
    let (high, low) = if base.to_bytes() > WSOL_MINT.to_bytes() {
        (base, WSOL_MINT)
    } else {
        (WSOL_MINT, base)
    };
    check(
        key == derive(
            PROGRAM_ID,
            &[b"pool", config.as_ref(), high.as_ref(), low.as_ref()],
        ),
        "incorrect pool PDA",
    )?;
    check(
        from_contract(pool.base_vault)
            == derive(PROGRAM_ID, &[b"token_vault", base.as_ref(), key.as_ref()]),
        "incorrect base vault PDA",
    )?;
    check(
        from_contract(pool.quote_vault)
            == derive(
                PROGRAM_ID,
                &[b"token_vault", WSOL_MINT.as_ref(), key.as_ref()],
            ),
        "incorrect quote vault PDA",
    )?;
    check(pool.sqrt_price > 0, "invalid pool sqrt price")
}

fn validate_config(config: &PoolConfig) -> Result<()> {
    check(
        from_contract(config.quote_mint) == WSOL_MINT,
        "only WSOL quote pools are supported",
    )?;
    check(
        config.token_type == 0 && config.quote_token_flag == 0,
        "Token-2022 is unsupported",
    )?;
    check(
        config.collect_fee_mode == 0,
        "only quote-token fees are supported",
    )?;
    check(config.activation_type <= 1, "invalid activation type")?;
    check(
        matches!(config.version, 1 | 2),
        "unsupported config version",
    )?;
    let fee = &config.pool_fees;
    check(
        !fee.dynamic_fee.is_dynamic_fee_enable(),
        "dynamic fees are unsupported",
    )?;
    check(
        fee.base_fee.base_fee_mode == 0
            && fee.base_fee.first_factor == 0
            && fee.base_fee.second_factor == 0
            && fee.base_fee.third_factor == 0,
        "only flat fees are supported",
    )?;
    check(
        matches!(fee.base_fee.cliff_fee_numerator, 10_000_000 | 20_000_000),
        "unsupported fee tier",
    )?;
    check(
        config.creator_trading_fee_percentage == 50,
        "unsupported creator fee share",
    )?;
    check(
        config.migration_quote_threshold > 0
            && config.migration_sqrt_price > config.sqrt_start_price
            && config.sqrt_start_price > 0,
        "invalid curve limits",
    )?;
    let mut previous = config.sqrt_start_price;
    let mut ended = false;
    let mut count = 0;
    for point in &config.curve {
        if point.liquidity == 0 {
            ended = true;
            continue;
        }
        check(
            !ended && point.sqrt_price > previous,
            "invalid curve point ordering",
        )?;
        previous = point.sqrt_price;
        count += 1;
    }
    check(
        count > 0 && previous >= config.migration_sqrt_price,
        "curve does not cover migration price",
    )
}

fn validate_mint(account: &impl ReadableAccount, decimals: u8) -> Result<()> {
    check(
        account.owner() == &TOKEN_PROGRAM_ID && !account.executable(),
        "mint must use legacy SPL Token",
    )?;
    let data = account.data();
    check(data.len() == 82, "mint extensions are unsupported")?;
    check(
        data[45] == 1 && data[44] == decimals,
        "uninitialized mint or incorrect decimals",
    )?;
    check(
        u32::from_le_bytes(data[46..50].try_into().unwrap()) == 0,
        "freeze authority is unsupported",
    )
}

fn validate_vault(account: &impl ReadableAccount, mint: Pubkey) -> Result<u64> {
    check(
        account.owner() == &TOKEN_PROGRAM_ID && !account.executable(),
        "vault must use legacy SPL Token",
    )?;
    let data = account.data();
    check(data.len() == 165, "vault extensions are unsupported")?;
    check(&data[..32] == mint.as_ref(), "incorrect vault mint")?;
    check(
        &data[32..64] == pool_authority().as_ref(),
        "incorrect vault authority",
    )?;
    check(data[108] == 1, "vault is frozen or uninitialized")?;
    check(
        u32::from_le_bytes(data[72..76].try_into().unwrap()) == 0,
        "delegated vault is unsupported",
    )?;
    check(
        u32::from_le_bytes(data[129..133].try_into().unwrap()) == 0,
        "vault close authority is unsupported",
    )?;
    let native = u32::from_le_bytes(data[109..113].try_into().unwrap());
    check(
        native == u32::from(mint == WSOL_MINT),
        "incorrect vault native-token state",
    )?;
    Ok(u64::from_le_bytes(data[64..72].try_into().unwrap()))
}

impl EvolutAmm {
    fn current_time(&self) -> Result<(u64, u64, u64)> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| AmmError::from("config is not loaded"))?;
        let timestamp = u64::try_from(self.clock.unix_timestamp.load(Ordering::Relaxed))
            .map_err(|_| AmmError::from("negative clock timestamp"))?;
        let slot = self.clock.slot.load(Ordering::Relaxed);
        let point = if config.activation_type == 0 {
            slot
        } else {
            timestamp
        };
        Ok((timestamp, slot, point))
    }

    fn require_active(&self) -> Result<&PoolConfig> {
        check(self.ready, "pool dependencies are not fully loaded")?;
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| AmmError::from("config is not loaded"))?;
        check(
            self.pool.is_migrated == 0
                && self.pool.migration_progress == 0
                && !self
                    .pool
                    .is_curve_complete(config.migration_quote_threshold),
            "pool is completed or migrated",
        )?;
        check(
            self.current_time()?.2 >= self.pool.activation_point,
            "pool is not activated",
        )?;
        Ok(config)
    }

    fn direction(&self, input: Pubkey, output: Pubkey) -> Result<bool> {
        let base = from_contract(self.pool.base_mint);
        if input == base && output == WSOL_MINT {
            Ok(true)
        } else if input == WSOL_MINT && output == base {
            Ok(false)
        } else {
            Err(AmmError::from("invalid input/output mint pair"))
        }
    }

    /// Build one top-level native swap; creates no token accounts and wraps/unwraps no SOL.
    pub fn native_swap_instruction(&self, params: &SwapParams) -> Result<Instruction> {
        let metas = self.get_swap_and_account_metas(params)?;
        Ok(Instruction {
            program_id: PROGRAM_ID,
            accounts: metas.account_metas,
            data: encode_native_swap(params.in_amount, params.out_amount),
        })
    }
}

impl Amm for EvolutAmm {
    fn from_keyed_account(account: &KeyedAccount, context: &AmmContext) -> Result<Self> {
        check(
            dynamic_bonding_curve::ID.to_bytes() == PROGRAM_ID.to_bytes(),
            "contract dependency must enable mainnet",
        )?;
        let pool = decode::<VirtualPool>(&account.account, POOL_ACCOUNT_LEN)?;
        validate_pool(account.key, &pool)?;
        Ok(Self {
            key: account.key,
            pool,
            config: None,
            clock: context.clock_ref.clone(),
            base_vault_amount: 0,
            quote_vault_amount: 0,
            ready: false,
        })
    }

    fn label(&self) -> AmmLabel {
        Self::LABEL
    }
    fn program_id(&self) -> Pubkey {
        PROGRAM_ID
    }
    fn key(&self) -> Pubkey {
        self.key
    }
    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        vec![from_contract(self.pool.base_mint), WSOL_MINT]
    }
    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        vec![
            self.key,
            from_contract(self.pool.config),
            from_contract(self.pool.base_mint),
            WSOL_MINT,
            from_contract(self.pool.base_vault),
            from_contract(self.pool.quote_vault),
        ]
    }

    fn update(&mut self, provider: impl AccountProvider) -> Result<()> {
        // Never permit a failed refresh to leave a previously valid quote active.
        self.ready = false;
        self.config = None;
        let pool = decode::<VirtualPool>(&provider.try_get(&self.key)?, POOL_ACCOUNT_LEN)?;
        validate_pool(self.key, &pool)?;
        self.pool = pool;
        // Install the new pool first so a missing dependency can be discovered
        // through the next get_accounts_to_update call.
        let config = decode::<PoolConfig>(
            &provider.try_get(&from_contract(pool.config))?,
            CONFIG_ACCOUNT_LEN,
        )?;
        validate_config(&config)?;
        validate_mint(
            &provider.try_get(&from_contract(pool.base_mint))?,
            config.token_decimal,
        )?;
        validate_mint(&provider.try_get(&WSOL_MINT)?, 9)?;
        let base_balance = validate_vault(
            &provider.try_get(&from_contract(pool.base_vault))?,
            from_contract(pool.base_mint),
        )?;
        let quote_balance = validate_vault(
            &provider.try_get(&from_contract(pool.quote_vault))?,
            WSOL_MINT,
        )?;
        let base_required = pool
            .base_reserve
            .checked_add(pool.protocol_base_fee)
            .and_then(|v| v.checked_add(pool.partner_base_fee))
            .and_then(|v| v.checked_add(pool.creator_base_fee));
        let quote_required = pool
            .quote_reserve
            .checked_add(pool.protocol_quote_fee)
            .and_then(|v| v.checked_add(pool.partner_quote_fee))
            .and_then(|v| v.checked_add(pool.creator_quote_fee));
        check(
            base_required.is_some_and(|n| base_balance >= n)
                && quote_required.is_some_and(|n| quote_balance >= n),
            "vault balances do not cover reserves and fees",
        )?;
        self.base_vault_amount = base_balance;
        self.quote_vault_amount = quote_balance;
        self.config = Some(config);
        self.ready = true;
        Ok(())
    }

    fn quote(&self, params: &QuoteParams) -> Result<Quote> {
        quote::exact_in(self, params)
    }

    fn get_swap_and_account_metas(&self, params: &SwapParams) -> Result<SwapAndAccountMetas> {
        self.require_active()?;
        check(
            params.swap_mode == SwapMode::ExactIn && params.in_amount > 0,
            "only positive ExactIn swaps are supported",
        )?;
        self.direction(params.source_mint, params.destination_mint)?;
        check(
            params.token_transfer_authority != Pubkey::default()
                && params.source_token_account != Pubkey::default()
                && params.destination_token_account != Pubkey::default(),
            "missing transfer authority or token accounts",
        )?;
        check(
            params.source_token_account != params.destination_token_account,
            "input and output token accounts must differ",
        )?;
        check(
            params.quote_mint_to_referrer.is_none_or(|r| r.is_empty()),
            "referrals are unsupported",
        )?;
        Ok(SwapAndAccountMetas {
            swap: Swap::Placeholder { data: None },
            account_metas: vec![
                AccountMeta::new_readonly(pool_authority(), false),
                AccountMeta::new_readonly(from_contract(self.pool.config), false),
                AccountMeta::new(self.key, false),
                AccountMeta::new(params.source_token_account, false),
                AccountMeta::new(params.destination_token_account, false),
                AccountMeta::new(from_contract(self.pool.base_vault), false),
                AccountMeta::new(from_contract(self.pool.quote_vault), false),
                AccountMeta::new_readonly(from_contract(self.pool.base_mint), false),
                AccountMeta::new_readonly(WSOL_MINT, false),
                AccountMeta::new_readonly(params.token_transfer_authority, true),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                // Anchor's absent optional account sentinel is this program, not
                // the Jupiter program and not an omitted element.
                AccountMeta::new_readonly(PROGRAM_ID, false),
                AccountMeta::new_readonly(event_authority(), false),
                AccountMeta::new_readonly(PROGRAM_ID, false),
            ],
        })
    }

    fn has_dynamic_accounts(&self) -> bool {
        true
    }
    fn requires_update_for_reserve_mints(&self) -> bool {
        false
    }
    fn get_accounts_len(&self) -> usize {
        15
    }
    fn underlying_liquidities(&self) -> Option<HashSet<Pubkey>> {
        Some(HashSet::from([self.key]))
    }
    fn is_active(&self) -> bool {
        self.require_active().is_ok()
    }
    fn program_dependencies(&self) -> Vec<(Pubkey, String)> {
        // The official kit's LiteSVM already supplies the SPL Token and ATA
        // programs. No additional external executable is used by native swaps.
        vec![]
    }
}
