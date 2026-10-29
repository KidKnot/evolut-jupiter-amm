use anchor_lang::prelude::*;

/// A replaceable automation signer, not an admin, fee recipient or mint authority.
/// Only the existing cold-admin instructions can initialize or replace it.
#[account]
#[derive(InitSpace)]
pub struct OperationalAuthority {
    pub operator: Pubkey,
}

impl OperationalAuthority {
    pub const SEED: &'static [u8] = b"operational_authority";
}
