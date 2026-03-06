//! Parameter sweep: Cartesian product of config ranges with parallel execution.
//!
//! [`SweepConfig`] defines which strategy parameters to sweep over.
//! [`run_sweep`] generates all combinations and runs each backtest in parallel
//! using Rayon. Results are deterministic regardless of thread count.

use rayon::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::{Candle, Instrument};
use crate::strategy::config::StrategyConfig;

use super::engine::run_backtest;
use super::result::BacktestResult;
use super::stats::BacktestStats;

/// Configuration for a parameter sweep.
///
/// Each `Vec` field specifies the values to sweep for that parameter.
/// An empty `Vec` means "use the base config value" (no sweep on that axis).
///
/// The total number of combinations is the Cartesian product of all
/// non-empty sweep fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SweepConfig {
    /// Stop loss distances to sweep (points).
    pub sl_fixed_points: Vec<Decimal>,

    /// Entry offset distances to sweep (points).
    pub entry_offset_points: Vec<Decimal>,

    /// Trailing stop distances to sweep (points).
    pub trailing_stop_distance: Vec<Decimal>,

    /// Add-to-winners intervals to sweep (points).
    pub add_every_points: Vec<Decimal>,

    /// Signal bar indices to sweep (1-based candle position after open).
    pub signal_bar_index: Vec<u8>,

    /// Number of threads for the sweep. `0` means use all available CPU cores.
    pub parallel_threads: u8,
}

/// A single result from a parameter sweep, pairing the config with its stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepResult {
    /// The full backtest result for this parameter combination.
    pub result: BacktestResult,
    /// The configuration used for this run (for comparison).
    pub config: StrategyConfig,
}

impl SweepConfig {
    /// Generate all parameter combinations as concrete [`StrategyConfig`] values.
    ///
    /// For each sweep field that is non-empty, every value is combined with
    /// every value from every other non-empty field (Cartesian product).
    /// Empty fields use the corresponding value from `base`.
    ///
    /// The output order is deterministic: it iterates through dimensions in
    /// field declaration order, inner dimensions varying fastest.
    #[must_use]
    pub fn combinations(&self, base: &StrategyConfig) -> Vec<StrategyConfig> {
        let sl_values = if self.sl_fixed_points.is_empty() {
            vec![base.sl_fixed_points]
        } else {
            self.sl_fixed_points.clone()
        };

        let entry_values = if self.entry_offset_points.is_empty() {
            vec![base.entry_offset_points]
        } else {
            self.entry_offset_points.clone()
        };

        let trail_values = if self.trailing_stop_distance.is_empty() {
            vec![base.trailing_stop_distance]
        } else {
            self.trailing_stop_distance.clone()
        };

        let add_values = if self.add_every_points.is_empty() {
            vec![base.add_every_points]
        } else {
            self.add_every_points.clone()
        };

        let bar_values = if self.signal_bar_index.is_empty() {
            vec![base.signal_bar_index]
        } else {
            self.signal_bar_index.clone()
        };

        let mut configs = Vec::with_capacity(
            sl_values.len()
                * entry_values.len()
                * trail_values.len()
                * add_values.len()
                * bar_values.len(),
        );

        for &sl in &sl_values {
            for &entry in &entry_values {
                for &trail in &trail_values {
                    for &add in &add_values {
                        for &bar in &bar_values {
                            let mut config = base.clone();
                            config.sl_fixed_points = sl;
                            config.entry_offset_points = entry;
                            config.trailing_stop_distance = trail;
                            config.add_every_points = add;
                            config.signal_bar_index = bar;
                            configs.push(config);
                        }
                    }
                }
            }
        }

        configs
    }

    /// Total number of parameter combinations that will be tested.
    #[must_use]
    pub fn total_combinations(&self, base: &StrategyConfig) -> usize {
        let sl = if self.sl_fixed_points.is_empty() {
            1
        } else {
            self.sl_fixed_points.len()
        };
        let entry = if self.entry_offset_points.is_empty() {
            1
        } else {
            self.entry_offset_points.len()
        };
        let trail = if self.trailing_stop_distance.is_empty() {
            1
        } else {
            self.trailing_stop_distance.len()
        };
        let add = if self.add_every_points.is_empty() {
            1
        } else {
            self.add_every_points.len()
        };
        let bar = if self.signal_bar_index.is_empty() {
            1
        } else {
            self.signal_bar_index.len()
        };
        let _ = base; // used only for consistency with combinations() signature
        sl * entry * trail * add * bar
    }
}

/// Run a parameter sweep: generate all combinations and backtest each in parallel.
///
/// # Arguments
///
/// * `candles` - All candle data for the instrument across the date range.
/// * `instrument` - The trading instrument.
/// * `base_config` - The base configuration; sweep fields override specific params.
/// * `sweep` - The sweep configuration defining which parameters to vary.
///
/// # Returns
///
/// A `Vec<SweepResult>` with one entry per parameter combination, in the
/// same deterministic order as [`SweepConfig::combinations`].
///
/// Results are deterministic regardless of thread count because each
/// backtest is independent and produces identical output for the same inputs.
#[must_use]
pub fn run_sweep(
    candles: &[Candle],
    instrument: Instrument,
    base_config: &StrategyConfig,
    sweep: &SweepConfig,
) -> Vec<SweepResult> {
    let configs = sweep.combinations(base_config);

    // Configure thread pool if a specific thread count is requested.
    if sweep.parallel_threads > 0 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(sweep.parallel_threads as usize)
            .build();

        if let Ok(pool) = pool {
            return pool.install(|| run_sweep_parallel(candles, instrument, &configs));
        }
    }

    // Default: use the global Rayon thread pool (all cores).
    run_sweep_parallel(candles, instrument, &configs)
}

/// Execute backtests in parallel using Rayon's `par_iter`.
fn run_sweep_parallel(
    candles: &[Candle],
    instrument: Instrument,
    configs: &[StrategyConfig],
) -> Vec<SweepResult> {
    configs
        .par_iter()
        .map(|config| {
            let result = run_backtest(candles, instrument, config);
            SweepResult {
                config: config.clone(),
                result,
            }
        })
        .collect()
}

/// Find the best result from a sweep by a given metric.
///
/// # Arguments
///
/// * `results` - The sweep results to search.
/// * `metric` - A function that extracts the metric to maximise from the stats.
///
/// Returns `None` if `results` is empty.
#[must_use]
pub fn best_by<F>(results: &[SweepResult], metric: F) -> Option<&SweepResult>
where
    F: Fn(&BacktestStats) -> f64,
{
    results.iter().max_by(|a, b| {
        let ma = metric(&a.result.stats);
        let mb = metric(&b.result.stats);
        ma.partial_cmp(&mb).unwrap_or(std::cmp::Ordering::Equal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn base_config() -> StrategyConfig {
        StrategyConfig::default()
    }

    // -- combinations() tests --

    #[test]
    fn test_empty_sweep_produces_single_config() {
        let sweep = SweepConfig::default();
        let combos = sweep.combinations(&base_config());
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].sl_fixed_points, dec!(40));
        assert_eq!(combos[0].entry_offset_points, dec!(2));
    }

    #[test]
    fn test_single_dimension_sweep() {
        let sweep = SweepConfig {
            sl_fixed_points: vec![dec!(20), dec!(30), dec!(40)],
            ..Default::default()
        };
        let combos = sweep.combinations(&base_config());
        assert_eq!(combos.len(), 3);
        assert_eq!(combos[0].sl_fixed_points, dec!(20));
        assert_eq!(combos[1].sl_fixed_points, dec!(30));
        assert_eq!(combos[2].sl_fixed_points, dec!(40));
        // Other params unchanged
        assert_eq!(combos[0].entry_offset_points, dec!(2));
    }

    #[test]
    fn test_two_dimension_sweep_cartesian_product() {
        let sweep = SweepConfig {
            sl_fixed_points: vec![dec!(20), dec!(40)],
            entry_offset_points: vec![dec!(1), dec!(3)],
            ..Default::default()
        };
        let combos = sweep.combinations(&base_config());
        assert_eq!(combos.len(), 4); // 2 x 2

        // sl=20, entry=1
        assert_eq!(combos[0].sl_fixed_points, dec!(20));
        assert_eq!(combos[0].entry_offset_points, dec!(1));
        // sl=20, entry=3
        assert_eq!(combos[1].sl_fixed_points, dec!(20));
        assert_eq!(combos[1].entry_offset_points, dec!(3));
        // sl=40, entry=1
        assert_eq!(combos[2].sl_fixed_points, dec!(40));
        assert_eq!(combos[2].entry_offset_points, dec!(1));
        // sl=40, entry=3
        assert_eq!(combos[3].sl_fixed_points, dec!(40));
        assert_eq!(combos[3].entry_offset_points, dec!(3));
    }

    #[test]
    fn test_five_dimension_sweep() {
        let sweep = SweepConfig {
            sl_fixed_points: vec![dec!(20), dec!(40)],
            entry_offset_points: vec![dec!(1), dec!(2)],
            trailing_stop_distance: vec![dec!(25), dec!(30)],
            add_every_points: vec![dec!(50)],
            signal_bar_index: vec![1, 2, 3],
            parallel_threads: 0,
        };
        let combos = sweep.combinations(&base_config());
        // 2 * 2 * 2 * 1 * 3 = 24
        assert_eq!(combos.len(), 24);
    }

    #[test]
    fn test_total_combinations_matches_actual() {
        let sweep = SweepConfig {
            sl_fixed_points: vec![dec!(20), dec!(30), dec!(40)],
            entry_offset_points: vec![dec!(1), dec!(2)],
            signal_bar_index: vec![1, 2],
            ..Default::default()
        };
        let base = base_config();
        let expected = sweep.total_combinations(&base);
        let actual = sweep.combinations(&base).len();
        assert_eq!(expected, actual);
        assert_eq!(expected, 12); // 3 * 2 * 1 * 1 * 2
    }

    #[test]
    fn test_combinations_preserve_base_config_fields() {
        let base = StrategyConfig {
            initial_capital: dec!(200000),
            position_size: dec!(2),
            ..StrategyConfig::default()
        };
        let sweep = SweepConfig {
            sl_fixed_points: vec![dec!(30)],
            ..Default::default()
        };
        let combos = sweep.combinations(&base);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].sl_fixed_points, dec!(30));
        assert_eq!(combos[0].initial_capital, dec!(200000));
        assert_eq!(combos[0].position_size, dec!(2));
    }

    #[test]
    fn test_signal_bar_index_sweep() {
        let sweep = SweepConfig {
            signal_bar_index: vec![1, 2, 3],
            ..Default::default()
        };
        let combos = sweep.combinations(&base_config());
        assert_eq!(combos.len(), 3);
        assert_eq!(combos[0].signal_bar_index, 1);
        assert_eq!(combos[1].signal_bar_index, 2);
        assert_eq!(combos[2].signal_bar_index, 3);
    }

    #[test]
    fn test_sweep_config_serde_roundtrip() {
        let sweep = SweepConfig {
            sl_fixed_points: vec![dec!(20), dec!(40)],
            entry_offset_points: vec![dec!(1)],
            parallel_threads: 4,
            ..Default::default()
        };
        let json = serde_json::to_string(&sweep).unwrap();
        let parsed: SweepConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sl_fixed_points, sweep.sl_fixed_points);
        assert_eq!(parsed.entry_offset_points, sweep.entry_offset_points);
        assert_eq!(parsed.parallel_threads, 4);
    }

    #[test]
    fn test_default_sweep_config() {
        let sweep = SweepConfig::default();
        assert!(sweep.sl_fixed_points.is_empty());
        assert!(sweep.entry_offset_points.is_empty());
        assert!(sweep.trailing_stop_distance.is_empty());
        assert!(sweep.add_every_points.is_empty());
        assert!(sweep.signal_bar_index.is_empty());
        assert_eq!(sweep.parallel_threads, 0);
    }
}
