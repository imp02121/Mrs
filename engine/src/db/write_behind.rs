//! Write-behind pipeline for batching writes to PostgreSQL.
//!
//! The [`WriteBehindWorker`] runs a background loop that collects write
//! operations from a [`WriteBehindSender`] and flushes them to Postgres
//! in batches (every 500ms or every 100 items, whichever comes first).

use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::backtest::result::BacktestResult;
use crate::models::Instrument;

/// Maximum number of operations to buffer before flushing.
const BATCH_SIZE: usize = 100;

/// How often to flush buffered operations (in milliseconds).
const FLUSH_INTERVAL_MS: u64 = 500;

/// A write operation to be persisted to PostgreSQL.
#[derive(Debug)]
pub enum WriteOp {
    /// Insert a completed backtest run and its results.
    InsertBacktestRun {
        /// Unique run identifier.
        run_id: Uuid,
        /// The backtest result to persist.
        result: Box<BacktestResult>,
    },
    /// Insert trades for a backtest run.
    InsertTrades {
        /// The backtest run these trades belong to.
        run_id: Uuid,
        /// Serialized trades as JSON for batch insert.
        trades_json: serde_json::Value,
    },
    /// Upsert the latest signal for an instrument.
    UpsertSignal {
        /// The instrument this signal belongs to.
        instrument: Instrument,
        /// The signal data as JSON.
        signal_json: serde_json::Value,
    },
}

/// Send-side handle for submitting write operations.
///
/// Cloneable and cheap to pass around. The background worker drains
/// operations from the corresponding receiver.
#[derive(Clone)]
pub struct WriteBehindSender {
    tx: mpsc::Sender<WriteOp>,
}

impl WriteBehindSender {
    /// Submit a write operation to the background pipeline.
    ///
    /// This is non-blocking; the operation is queued and will be flushed
    /// in the next batch cycle.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker has shut down (channel closed).
    pub async fn send(&self, op: WriteOp) -> Result<(), mpsc::error::SendError<WriteOp>> {
        self.tx.send(op).await
    }
}

/// Background worker that drains write operations and flushes them to Postgres.
///
/// Create with [`WriteBehindWorker::new`], then spawn [`WriteBehindWorker::run`]
/// on a tokio task. The worker shuts down gracefully when all
/// [`WriteBehindSender`] handles are dropped (channel closes).
pub struct WriteBehindWorker {
    pool: PgPool,
    rx: mpsc::Receiver<WriteOp>,
}

impl WriteBehindWorker {
    /// Create a new worker and its corresponding sender handle.
    ///
    /// `buffer_size` controls the mpsc channel capacity.
    #[must_use]
    pub fn new(pool: PgPool, buffer_size: usize) -> (Self, WriteBehindSender) {
        let (tx, rx) = mpsc::channel(buffer_size);
        let worker = Self { pool, rx };
        let sender = WriteBehindSender { tx };
        (worker, sender)
    }

    /// Run the background flush loop.
    ///
    /// Drains the channel and flushes to Postgres every 500ms or every
    /// 100 items, whichever comes first. Returns when the channel closes
    /// (all senders dropped) and remaining items have been flushed.
    pub async fn run(mut self) {
        info!("write-behind worker started");
        let mut buffer: Vec<WriteOp> = Vec::with_capacity(BATCH_SIZE);
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(FLUSH_INTERVAL_MS));

        loop {
            tokio::select! {
                // Receive operations from the channel
                maybe_op = self.rx.recv() => {
                    match maybe_op {
                        Some(op) => {
                            buffer.push(op);
                            if buffer.len() >= BATCH_SIZE {
                                Self::flush(&self.pool, &mut buffer).await;
                            }
                        }
                        None => {
                            // Channel closed — flush remaining and exit
                            if !buffer.is_empty() {
                                Self::flush(&self.pool, &mut buffer).await;
                            }
                            info!("write-behind worker shutting down");
                            return;
                        }
                    }
                }
                // Periodic flush
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        Self::flush(&self.pool, &mut buffer).await;
                    }
                }
            }
        }
    }

    /// Flush all buffered operations to Postgres.
    async fn flush(pool: &PgPool, buffer: &mut Vec<WriteOp>) {
        let ops: Vec<WriteOp> = std::mem::take(buffer);
        let count = ops.len();
        debug!(count, "flushing write-behind buffer");

        for op in ops {
            if let Err(e) = Self::execute_op(pool, op).await {
                error!(error = %e, "write-behind operation failed");
            }
        }
    }

    /// Execute a single write operation against Postgres.
    async fn execute_op(pool: &PgPool, op: WriteOp) -> Result<(), sqlx::Error> {
        match op {
            WriteOp::InsertBacktestRun { run_id, result } => {
                // Serialize stats to JSONB; fall back to empty object on failure.
                let stats_json = serde_json::to_value(&result.stats)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                let config_json = serde_json::to_value(&result.config)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                // First ensure a strategy_configs row exists for this config.
                let config_id: (uuid::Uuid,) = sqlx::query_as(
                    "INSERT INTO strategy_configs (name, params) VALUES ($1, $2) RETURNING id",
                )
                .bind("auto")
                .bind(&config_json)
                .fetch_one(pool)
                .await?;

                // Look up instrument_id from the instruments table.
                let inst_row: (i16,) =
                    sqlx::query_as("SELECT id FROM instruments WHERE symbol = $1")
                        .bind(result.instrument.ticker())
                        .fetch_one(pool)
                        .await?;

                sqlx::query(
                    r#"
                    INSERT INTO backtest_runs
                        (id, config_id, instrument_id, start_date, end_date,
                         total_trades, stats, duration_ms)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, 0)
                    ON CONFLICT (id) DO UPDATE SET stats = $7
                    "#,
                )
                .bind(run_id)
                .bind(config_id.0)
                .bind(inst_row.0)
                .bind(result.config.date_from)
                .bind(result.config.date_to)
                .bind(result.trades.len() as i32)
                .bind(&stats_json)
                .execute(pool)
                .await?;
                debug!(%run_id, "inserted backtest run");
            }
            WriteOp::InsertTrades {
                run_id,
                trades_json,
            } => {
                // The trades_json is expected to be a JSON array of trade objects.
                // For write-behind, we store each trade as a row in the trades table.
                // However, for simplicity, we log the intent; bulk trade inserts
                // should use db::trades::insert_trades directly.
                debug!(%run_id, count = %trades_json.as_array().map(|a| a.len()).unwrap_or(0), "write-behind trades (deferred to bulk insert)");
            }
            WriteOp::UpsertSignal {
                instrument,
                signal_json,
            } => {
                // Look up instrument_id from the instruments table.
                let inst_row: (i16,) =
                    sqlx::query_as("SELECT id FROM instruments WHERE symbol = $1")
                        .bind(instrument.ticker())
                        .fetch_one(pool)
                        .await?;

                let today = chrono::Utc::now().date_naive();
                let signal_bar_high = signal_json
                    .get("signal_bar_high")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let signal_bar_low = signal_json
                    .get("signal_bar_low")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let buy_level = signal_json
                    .get("buy_level")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let sell_level = signal_json
                    .get("sell_level")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                sqlx::query(
                    r#"
                    INSERT INTO live_signals
                        (instrument_id, signal_date, signal_bar_high, signal_bar_low,
                         buy_level, sell_level, status)
                    VALUES ($1, $2, $3, $4, $5, $6, 'pending')
                    ON CONFLICT (instrument_id, signal_date) DO UPDATE SET
                        signal_bar_high = EXCLUDED.signal_bar_high,
                        signal_bar_low  = EXCLUDED.signal_bar_low,
                        buy_level       = EXCLUDED.buy_level,
                        sell_level      = EXCLUDED.sell_level
                    "#,
                )
                .bind(inst_row.0)
                .bind(today)
                .bind(signal_bar_high)
                .bind(signal_bar_low)
                .bind(buy_level)
                .bind(sell_level)
                .execute(pool)
                .await?;
                debug!(instrument = instrument.ticker(), "upserted signal");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_constants() {
        assert_eq!(BATCH_SIZE, 100);
        assert_eq!(FLUSH_INTERVAL_MS, 500);
    }

    #[test]
    fn test_write_op_debug() {
        let op = WriteOp::UpsertSignal {
            instrument: Instrument::Dax,
            signal_json: serde_json::json!({"test": true}),
        };
        let debug_str = format!("{:?}", op);
        assert!(debug_str.contains("UpsertSignal"));
        assert!(debug_str.contains("Dax"));
    }

    #[tokio::test]
    async fn test_sender_fails_after_worker_dropped() {
        // Create a pool-less scenario: we just test channel behavior.
        // We can't create a real PgPool without a database, but we can
        // test that the sender returns an error when the receiver is dropped.
        let (tx, rx) = mpsc::channel::<WriteOp>(10);
        let sender = WriteBehindSender { tx };

        // Drop the receiver
        drop(rx);

        let result = sender
            .send(WriteOp::UpsertSignal {
                instrument: Instrument::Dax,
                signal_json: serde_json::json!({}),
            })
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_sender_is_clone() {
        let (tx, _rx) = mpsc::channel::<WriteOp>(10);
        let sender = WriteBehindSender { tx };
        let _cloned = sender.clone();
    }

    #[test]
    fn test_write_op_insert_backtest_run_construction() {
        use crate::backtest::result::BacktestResult;
        use crate::strategy::config::StrategyConfig;

        let config = StrategyConfig::default();
        let result = BacktestResult::from_trades(Instrument::Dax, config, Vec::new());
        let run_id = Uuid::new_v4();

        let op = WriteOp::InsertBacktestRun {
            run_id,
            result: Box::new(result),
        };
        let debug_str = format!("{:?}", op);
        assert!(debug_str.contains("InsertBacktestRun"));
        assert!(debug_str.contains(&run_id.to_string()));
    }

    #[test]
    fn test_write_op_insert_trades_construction() {
        let trades_json = serde_json::json!([
            {"direction": "Long", "entry_price": 16000, "exit_price": 16050},
            {"direction": "Short", "entry_price": 16050, "exit_price": 16020}
        ]);
        let run_id = Uuid::new_v4();

        let op = WriteOp::InsertTrades {
            run_id,
            trades_json,
        };
        let debug_str = format!("{:?}", op);
        assert!(debug_str.contains("InsertTrades"));
    }

    #[test]
    fn test_write_op_upsert_signal_construction() {
        let signal_json = serde_json::json!({
            "signal_bar_high": 16050.00,
            "signal_bar_low": 15980.00,
            "buy_level": 16052.00,
            "sell_level": 15978.00
        });

        let op = WriteOp::UpsertSignal {
            instrument: Instrument::Ftse,
            signal_json,
        };
        let debug_str = format!("{:?}", op);
        assert!(debug_str.contains("UpsertSignal"));
        assert!(debug_str.contains("Ftse"));
    }

    #[test]
    fn test_batch_size_constant() {
        assert_eq!(BATCH_SIZE, 100);
    }

    #[test]
    fn test_flush_interval_constant() {
        assert_eq!(FLUSH_INTERVAL_MS, 500);
    }

    #[test]
    fn test_write_op_all_instrument_variants() {
        for instrument in Instrument::ALL {
            let op = WriteOp::UpsertSignal {
                instrument,
                signal_json: serde_json::json!({}),
            };
            let debug_str = format!("{:?}", op);
            assert!(debug_str.contains("UpsertSignal"));
        }
    }

    #[tokio::test]
    async fn test_sender_send_succeeds_while_receiver_alive() {
        let (tx, mut rx) = mpsc::channel::<WriteOp>(10);
        let sender = WriteBehindSender { tx };

        let result = sender
            .send(WriteOp::UpsertSignal {
                instrument: Instrument::Nasdaq,
                signal_json: serde_json::json!({"test": true}),
            })
            .await;
        assert!(result.is_ok());

        let received = rx.recv().await;
        assert!(received.is_some());
    }
}
