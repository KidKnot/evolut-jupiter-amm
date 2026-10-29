use anchor_lang::prelude::*;
use crate::PoolError;

/// Lifetime amounts paid to [creator FeeKey, partner FeeKey], in token0/token1.
/// No close/reset instruction: unclaimed entitlement follows the current NFT.
#[account]
#[derive(InitSpace, Default)]
pub struct RaydiumCreatorFees {
    pub claimed: [[u128; 2]; 2],
}

impl RaydiumCreatorFees {
    pub const SEED: &'static [u8] = b"ray_creator_ledger";

    pub fn claimable(&self, balances: [u64; 2], side: u8) -> Result<[u64; 2]> {
        require!(side < 2, PoolError::InvalidInput);
        let mut result = [0; 2];
        for token in 0..2 {
            let total = self.claimed[0][token].checked_add(self.claimed[1][token])
                .and_then(|paid| paid.checked_add(balances[token] as u128))
                .ok_or(PoolError::MathOverflow)?;
            // Lifetime rounding, not per-call rounding: repeated 1-unit claims
            // cannot take the other side's half. Partner gets the odd unit.
            let entitlement = if side == 0 { total / 2 } else { total - total / 2 };
            result[token] = entitlement.checked_sub(self.claimed[side as usize][token])
                .and_then(|amount| u64::try_from(amount).ok())
                .ok_or(PoolError::MathOverflow)?;
            require!(result[token] <= balances[token], PoolError::InvalidAccount);
        }
        Ok(result)
    }

    pub fn record(&mut self, side: u8, amounts: [u64; 2]) -> Result<()> {
        require!(side < 2, PoolError::InvalidInput);
        for token in 0..2 {
            self.claimed[side as usize][token] = self.claimed[side as usize][token]
                .checked_add(amounts[token] as u128).ok_or(PoolError::MathOverflow)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_claim_order_conserves_both_tokens_and_replay_pays_zero() {
        for first in [0, 1] {
            let mut ledger = RaydiumCreatorFees::default();
            let mut balances = [101, 70];
            for side in [first, 1 - first] {
                let amounts = ledger.claimable(balances, side).unwrap();
                ledger.record(side, amounts).unwrap();
                for t in 0..2 { balances[t] -= amounts[t]; }
            }
            assert_eq!(balances, [0, 0]);
            assert_eq!(ledger.claimed, [[50, 35], [51, 35]]);
            assert_eq!(ledger.claimable(balances, first).unwrap(), [0, 0]);
        }
    }

    #[test]
    fn repeated_dust_collections_cannot_steal_the_other_half() {
        let mut ledger = RaydiumCreatorFees::default();
        let mut balances = [0, 0];
        for _ in 0..1001 {
            balances[0] += 1;
            let amounts = ledger.claimable(balances, 1).unwrap();
            ledger.record(1, amounts).unwrap();
            balances[0] -= amounts[0];
        }
        assert_eq!(ledger.claimed[1][0], 501);
        assert_eq!(ledger.claimable(balances, 0).unwrap(), [500, 0]);
    }

    #[test]
    fn externally_collected_fees_and_donations_are_not_lost() {
        let mut ledger = RaydiumCreatorFees::default();
        let first = ledger.claimable([100, 200], 0).unwrap();
        ledger.record(0, first).unwrap();
        // An outsider triggers Raydium's permissionless collection before us.
        assert_eq!(ledger.claimable([70, 140], 1).unwrap(), [60, 120]);
        assert_eq!(ledger.claimable([70, 140], 0).unwrap(), [10, 20]);
        assert!(ledger.claimable([70, 140], 2).is_err());
    }
}
