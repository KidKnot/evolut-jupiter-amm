use dynamic_bonding_curve::state::PoolConfig;

fn policy(threshold: u64, version: u8, fee: u64) -> PoolConfig {
    let mut c = PoolConfig { version, migration_quote_threshold: threshold,
        max_setup_lamports: 200_000_000, ..Default::default() };
    c._padding[..8].copy_from_slice(&fee.to_le_bytes());
    c
}

#[test]
fn production_and_test_conserve_raised_sol() {
    for (threshold, fee, expected) in [
        (85_000_000_000, 800_000_000, 84_008_730_080),
        (500_000_000, 0, 308_730_080),
    ] {
        let c = policy(threshold, 2, fee);
        let actual = 191_269_920;
        assert_eq!(c.graduation_quote_deposit(actual).unwrap(), expected);
        assert_eq!(expected + fee + actual, threshold);
    }
}

#[test]
fn legacy_configs_never_inherit_revenue() {
    let c = policy(85_000_000_000, 1, 0);
    assert_eq!(c.graduation_fee_lamports().unwrap(), 0);
    assert_eq!(c.graduation_quote_deposit(200_000_000).unwrap(), 84_800_000_000);
    assert!(policy(85_000_000_000, 1, 800_000_000).graduation_fee_lamports().is_err());
}

#[test]
fn malformed_or_unapproved_policies_and_excess_expense_fail() {
    for (threshold, version, fee) in [
        (500_000_000, 2, 800_000_000), (85_000_000_000, 2, 0),
        (85_000_000_000, 2, 800_000_001), (1_000_000_000, 2, 0),
        (85_000_000_000, 3, 800_000_000),
    ] { assert!(policy(threshold, version, fee).graduation_quote_deposit(1).is_err()); }
    let mut c = policy(85_000_000_000, 2, 800_000_000);
    assert!(c.graduation_quote_deposit(200_000_001).is_err());
    c._padding[14] = 1;
    assert!(c.graduation_fee_lamports().is_err());
    c._padding[14] = 0;
    c.max_setup_lamports = u64::MAX;
    assert!(c.graduation_quote_deposit(u64::MAX).is_err());
    assert!(c.graduation_quote_deposit(84_200_000_000).is_err());
}
