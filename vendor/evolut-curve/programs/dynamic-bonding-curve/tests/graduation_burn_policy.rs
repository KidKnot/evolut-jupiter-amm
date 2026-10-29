//! Exercise the same inventory/burn planner used by the graduation instruction.
use dynamic_bonding_curve::{
    instructions::partner::ix_create_config::TokenSupplyParams,
    state::PoolConfig,
};

fn policy(floor: u64) -> PoolConfig {
    PoolConfig { fixed_token_supply_flag: 1, pre_migration_token_supply: 1000,
        post_migration_token_supply: floor, ..Default::default() }
}

#[test]
fn burn_only_unused_inventory_protects_holders_liquidity_fees_and_donations() {
    let config = policy(0);
    // Initial 1000: holders own 800, ledger retains 200. Of those 200,
    // 120 go into the AMM; 80 remain unused. Vault additionally has fees/donations.
    for fees in [0u64, 1, 50] {
        for donation in [0u64, 1, 5000] {
            let vault = 80 + fees + donation;
            let (burn, claim) = config.graduation_burn_amounts(200, 0, 120, vault, fees).unwrap();
            assert_eq!(burn, 80);
            assert_eq!(claim, 0);
            assert_eq!(1000 - burn, 800 + 120);
            assert_eq!(vault - burn, fees + donation);
        }
    }
    assert_eq!(config.graduation_burn_amounts(120, 0, 120, 10, 10).unwrap(), (0, 0));
}

#[test]
fn existing_supply_floor_and_vesting_behavior_are_preserved() {
    assert_eq!(policy(950).graduation_burn_amounts(200, 0, 120, 80, 0).unwrap(), (50, 30));
    // Vesting already moved out must not become a second burn.
    assert_eq!(policy(0).graduation_burn_amounts(200, 50, 120, 30, 0).unwrap(), (30, 0));
    assert_eq!(policy(1000).graduation_burn_amounts(200, 0, 120, 80, 0).unwrap(), (0, 80));
}

#[test]
fn insufficient_vault_corrupt_inventory_and_overflows_abort() {
    let config = policy(0);
    assert!(config.graduation_burn_amounts(200, 0, 120, 89, 10).is_err());
    assert!(config.graduation_burn_amounts(200, 201, 0, 200, 0).is_err());
    assert!(config.graduation_burn_amounts(200, 0, 201, 200, 0).is_err());
    assert!(config.graduation_burn_amounts(1001, 0, 0, 1001, 0).is_err());
    assert!(config.graduation_burn_amounts(200, 0, 0, u64::MAX, u64::MAX).is_err());
}

#[test]
fn new_supply_policy_keeps_inventory_requirements_even_without_a_supply_floor() {
    let zero = TokenSupplyParams { pre_migration_token_supply: 1000, post_migration_token_supply: 0 };
    let legacy = TokenSupplyParams { post_migration_token_supply: 900, ..zero };
    if cfg!(feature = "mainnet") {
        assert!(zero.validate_supply_requirements(950, 900).is_ok());
        assert!(legacy.validate_supply_requirements(950, 900).is_err());
    } else {
        assert!(zero.validate_supply_requirements(950, 900).is_err());
        assert!(legacy.validate_supply_requirements(950, 900).is_ok());
    }
    for selected in [zero, legacy] {
        assert!(selected.validate_supply_requirements(1001, 900).is_err());
        assert!(selected.validate_supply_requirements(800, 900).is_err());
    }
}
