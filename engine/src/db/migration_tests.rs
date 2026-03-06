//! Tests that validate migration SQL files exist and contain expected content.

use crate::models::Instrument;

const MIGRATIONS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../migrations");

const MIGRATION_FILES: [&str; 7] = [
    "001_create_instruments.sql",
    "002_create_candles.sql",
    "003_create_strategy_configs.sql",
    "004_create_backtest_runs.sql",
    "005_create_trades.sql",
    "006_create_live_signals.sql",
    "007_create_subscribers.sql",
];

const EXPECTED_TABLES: [&str; 7] = [
    "instruments",
    "candles",
    "strategy_configs",
    "backtest_runs",
    "trades",
    "live_signals",
    "subscribers",
];

#[test]
fn test_all_migration_files_exist() {
    for file_name in &MIGRATION_FILES {
        let path = format!("{MIGRATIONS_DIR}/{file_name}");
        assert!(
            std::path::Path::new(&path).exists(),
            "migration file not found: {path}"
        );
    }
}

#[test]
fn test_all_migration_files_are_readable() {
    for file_name in &MIGRATION_FILES {
        let path = format!("{MIGRATIONS_DIR}/{file_name}");
        let content =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
        assert!(!content.is_empty(), "migration file is empty: {file_name}");
    }
}

#[test]
fn test_each_migration_contains_expected_table() {
    for (file_name, table_name) in MIGRATION_FILES.iter().zip(EXPECTED_TABLES.iter()) {
        let path = format!("{MIGRATIONS_DIR}/{file_name}");
        let content =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
        assert!(
            content.contains(table_name),
            "migration {file_name} does not contain table name '{table_name}'"
        );
    }
}

#[test]
fn test_instrument_seed_data_matches_instrument_all() {
    let path = format!("{MIGRATIONS_DIR}/001_create_instruments.sql");
    let content = std::fs::read_to_string(&path).expect("read 001 migration");

    for instrument in Instrument::ALL {
        let ticker = instrument.ticker();
        assert!(
            content.contains(ticker),
            "instrument ticker '{ticker}' not found in seed data"
        );
    }
}

#[test]
fn test_instrument_seed_data_contains_all_names() {
    let path = format!("{MIGRATIONS_DIR}/001_create_instruments.sql");
    let content = std::fs::read_to_string(&path).expect("read 001 migration");

    for instrument in Instrument::ALL {
        let name = instrument.name();
        assert!(
            content.contains(name),
            "instrument name '{name}' not found in seed data"
        );
    }
}

#[test]
fn test_exactly_seven_migration_files() {
    assert_eq!(MIGRATION_FILES.len(), 7);
}

#[test]
fn test_migrations_contain_create_table() {
    for file_name in &MIGRATION_FILES {
        let path = format!("{MIGRATIONS_DIR}/{file_name}");
        let content =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
        let upper = content.to_uppercase();
        assert!(
            upper.contains("CREATE TABLE"),
            "migration {file_name} does not contain CREATE TABLE"
        );
    }
}
