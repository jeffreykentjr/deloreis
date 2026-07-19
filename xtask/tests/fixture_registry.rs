#![allow(clippy::unwrap_used, clippy::expect_used)]
//! A file under fixtures/ that the registry does not list must fail the check.

use std::fs;

#[test]
fn stray_fixture_not_in_registry_fails() {
    let base = std::env::temp_dir().join(format!(
        "deloreis-fixreg-{}-{}",
        std::process::id(),
        line!()
    ));
    let _ = fs::remove_dir_all(&base);
    let contract = base.join("contracts/fantasy-worker");
    let golden = contract.join("fixtures/golden");
    let schemas = contract.join("schemas");
    fs::create_dir_all(&golden).unwrap();
    fs::create_dir_all(&schemas).unwrap();

    fs::write(
        contract.join("registry.yaml"),
        "authority: closed_fixture_registry\nfixtures: []\n",
    )
    .unwrap();
    fs::write(golden.join("stray-v1.json"), "{}\n").unwrap();

    let result = xtask::fixtures::run_fixtures(&contract, &schemas, false);
    assert!(
        result.is_err(),
        "a stray fixture must be rejected as unknown"
    );

    let _ = fs::remove_dir_all(&base);
}
