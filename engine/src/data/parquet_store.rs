//! Local Parquet file storage for candle data.
//!
//! Candles are partitioned by instrument and month into files at
//! `data/{instrument}/{YYYY-MM}.parquet` with Snappy compression.
//! This allows fast reads of specific date ranges without scanning
//! the entire dataset.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Float64Array, Int64Array, StringArray, TimestampMillisecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::models::{Candle, DateRange, Instrument};

use super::error::DataError;

/// Arrow schema used for candle Parquet files.
///
/// Columns:
/// - `timestamp`: Timestamp(Millisecond, UTC)
/// - `open`, `high`, `low`, `close`: Float64 (Parquet compatibility)
/// - `volume`: Int64
fn candle_schema() -> Schema {
    Schema::new(vec![
        Field::new(
            "timestamp",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            false,
        ),
        Field::new("open", DataType::Float64, false),
        Field::new("high", DataType::Float64, false),
        Field::new("low", DataType::Float64, false),
        Field::new("close", DataType::Float64, false),
        Field::new("volume", DataType::Int64, false),
        Field::new("instrument", DataType::Utf8, false),
    ])
}

/// Generates the filesystem path for a specific instrument and year-month.
///
/// Returns `{base_dir}/{instrument_ticker_lower}/{YYYY-MM}.parquet`.
fn parquet_path(base_dir: &Path, instrument: Instrument, year: i32, month: u32) -> PathBuf {
    base_dir
        .join(instrument.ticker().to_ascii_lowercase())
        .join(format!("{year:04}-{month:02}.parquet"))
}

/// Year-month key used for grouping candles into per-month files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct YearMonth {
    year: i32,
    month: u32,
}

impl From<DateTime<Utc>> for YearMonth {
    fn from(dt: DateTime<Utc>) -> Self {
        Self {
            year: dt.year(),
            month: dt.month(),
        }
    }
}

/// Local Parquet file store for OHLCV candle data.
///
/// Files are stored under `{base_dir}/{instrument}/{YYYY-MM}.parquet`.
/// Existing files for a given month are overwritten on write (the caller
/// should merge with existing data if incremental writes are desired).
pub struct ParquetStore {
    /// Root directory for data files (e.g. `data/`).
    base_dir: PathBuf,
}

impl ParquetStore {
    /// Create a new store rooted at `base_dir`.
    ///
    /// The directory will be created on first write if it does not exist.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Write candles to Parquet files, grouped by instrument and month.
    ///
    /// Creates directories as needed. Overwrites existing files for the
    /// same instrument/month combination.
    ///
    /// # Errors
    ///
    /// Returns [`DataError`] on I/O or Arrow/Parquet encoding failures.
    pub fn write_candles(&self, candles: &[Candle]) -> Result<usize, DataError> {
        if candles.is_empty() {
            return Ok(0);
        }

        // Group candles by (instrument, year-month).
        let mut groups: BTreeMap<(Instrument, YearMonth), Vec<&Candle>> = BTreeMap::new();
        for candle in candles {
            let ym = YearMonth::from(candle.timestamp);
            groups
                .entry((candle.instrument, ym))
                .or_default()
                .push(candle);
        }

        let mut total_written = 0usize;

        for ((instrument, ym), group) in &groups {
            let path = parquet_path(&self.base_dir, *instrument, ym.year, ym.month);

            // Ensure parent directory exists.
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| DataError::io(parent, e))?;
            }

            // Sort candles by timestamp for consistent file ordering.
            let mut sorted: Vec<&Candle> = group.clone();
            sorted.sort_by_key(|c| c.timestamp);

            let batch = candles_to_record_batch(&sorted)?;
            write_parquet_file(&path, &batch)?;

            total_written += sorted.len();
        }

        Ok(total_written)
    }

    /// Read candles for a specific instrument within a date range.
    ///
    /// Reads all relevant monthly Parquet files and filters candles
    /// to those falling within the date range. Returns candles sorted
    /// by timestamp.
    ///
    /// Missing files for a given month are silently skipped (treated
    /// as having no data for that period).
    ///
    /// # Errors
    ///
    /// Returns [`DataError`] on Parquet decoding or I/O failures.
    pub fn read_candles(
        &self,
        instrument: Instrument,
        range: DateRange,
    ) -> Result<Vec<Candle>, DataError> {
        let start_utc = range.start.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc());
        let end_utc = range
            .end
            .succ_opt()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|ndt| ndt.and_utc());

        let mut all_candles = Vec::new();

        // Iterate over each month in the range.
        let mut current = NaiveDate::from_ymd_opt(range.start.year(), range.start.month(), 1);
        let end_ym = YearMonth {
            year: range.end.year(),
            month: range.end.month(),
        };

        while let Some(date) = current {
            let ym = YearMonth {
                year: date.year(),
                month: date.month(),
            };
            if ym > end_ym {
                break;
            }

            let path = parquet_path(&self.base_dir, instrument, ym.year, ym.month);

            if path.exists() {
                let candles = read_parquet_file(&path, instrument)?;
                for candle in candles {
                    let include = match (start_utc, end_utc) {
                        (Some(s), Some(e)) => candle.timestamp >= s && candle.timestamp < e,
                        (Some(s), None) => candle.timestamp >= s,
                        (None, Some(e)) => candle.timestamp < e,
                        (None, None) => true,
                    };
                    if include {
                        all_candles.push(candle);
                    }
                }
            }

            // Advance to next month.
            current = if ym.month == 12 {
                NaiveDate::from_ymd_opt(ym.year + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(ym.year, ym.month + 1, 1)
            };
        }

        all_candles.sort_by_key(|c| c.timestamp);
        Ok(all_candles)
    }
}

/// Convert a slice of candles into an Arrow [`RecordBatch`].
fn candles_to_record_batch(candles: &[&Candle]) -> Result<RecordBatch, DataError> {
    let schema = Arc::new(candle_schema());

    let timestamps: Vec<i64> = candles
        .iter()
        .map(|c| c.timestamp.timestamp_millis())
        .collect();

    let opens: Vec<f64> = candles
        .iter()
        .map(|c| c.open.to_f64().unwrap_or(0.0))
        .collect();

    let highs: Vec<f64> = candles
        .iter()
        .map(|c| c.high.to_f64().unwrap_or(0.0))
        .collect();

    let lows: Vec<f64> = candles
        .iter()
        .map(|c| c.low.to_f64().unwrap_or(0.0))
        .collect();

    let closes: Vec<f64> = candles
        .iter()
        .map(|c| c.close.to_f64().unwrap_or(0.0))
        .collect();

    let volumes: Vec<i64> = candles.iter().map(|c| c.volume).collect();

    let instruments: Vec<&str> = candles.iter().map(|c| c.instrument.ticker()).collect();

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(TimestampMillisecondArray::from(timestamps).with_timezone("UTC")),
            Arc::new(Float64Array::from(opens)),
            Arc::new(Float64Array::from(highs)),
            Arc::new(Float64Array::from(lows)),
            Arc::new(Float64Array::from(closes)),
            Arc::new(Int64Array::from(volumes)),
            Arc::new(StringArray::from(instruments)),
        ],
    )?;

    Ok(batch)
}

/// Write a single [`RecordBatch`] to a Parquet file with Snappy compression.
fn write_parquet_file(path: &Path, batch: &RecordBatch) -> Result<(), DataError> {
    let file = fs::File::create(path).map_err(|e| DataError::io(path, e))?;

    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;
    writer.write(batch)?;
    writer.close()?;

    Ok(())
}

/// Read candles from a Parquet file for a specific instrument.
fn read_parquet_file(path: &Path, instrument: Instrument) -> Result<Vec<Candle>, DataError> {
    let file = fs::File::open(path).map_err(|e| DataError::io(path, e))?;

    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;

    let mut candles = Vec::new();

    for batch_result in reader {
        let batch = batch_result?;
        let num_rows = batch.num_rows();

        let ts_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<TimestampMillisecondArray>()
            .ok_or_else(|| DataError::Validation("timestamp column type mismatch".into()))?;

        let open_col = batch
            .column(1)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataError::Validation("open column type mismatch".into()))?;

        let high_col = batch
            .column(2)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataError::Validation("high column type mismatch".into()))?;

        let low_col = batch
            .column(3)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataError::Validation("low column type mismatch".into()))?;

        let close_col = batch
            .column(4)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataError::Validation("close column type mismatch".into()))?;

        let vol_col = batch
            .column(5)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| DataError::Validation("volume column type mismatch".into()))?;

        for i in 0..num_rows {
            let ts_millis = ts_col.value(i);
            let timestamp = DateTime::from_timestamp_millis(ts_millis).ok_or_else(|| {
                DataError::Validation(format!("invalid timestamp millis: {ts_millis}"))
            })?;

            candles.push(Candle {
                instrument,
                timestamp,
                open: Decimal::try_from(open_col.value(i)).unwrap_or(Decimal::ZERO),
                high: Decimal::try_from(high_col.value(i)).unwrap_or(Decimal::ZERO),
                low: Decimal::try_from(low_col.value(i)).unwrap_or(Decimal::ZERO),
                close: Decimal::try_from(close_col.value(i)).unwrap_or(Decimal::ZERO),
                volume: vol_col.value(i),
            });
        }
    }

    Ok(candles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use tempfile::TempDir;

    fn make_candle(instrument: Instrument, year: i32, month: u32, day: u32, hour: u32) -> Candle {
        let ts = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
            .and_utc();
        Candle {
            instrument,
            timestamp: ts,
            open: dec!(16000.50),
            high: dec!(16050.00),
            low: dec!(15980.25),
            close: dec!(16030.75),
            volume: 12345,
        }
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let candles = vec![
            make_candle(Instrument::Dax, 2024, 1, 15, 8),
            make_candle(Instrument::Dax, 2024, 1, 15, 9),
            make_candle(Instrument::Dax, 2024, 1, 16, 8),
        ];

        let written = store.write_candles(&candles).unwrap();
        assert_eq!(written, 3);

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(read.len(), 3);
        assert_eq!(read[0].open, dec!(16000.50));
        assert_eq!(read[0].volume, 12345);
    }

    #[test]
    fn test_write_empty_candles() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let written = store.write_candles(&[]).unwrap();
        assert_eq!(written, 0);
    }

    #[test]
    fn test_read_missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert!(read.is_empty());
    }

    #[test]
    fn test_multiple_instruments_separate_files() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let candles = vec![
            make_candle(Instrument::Dax, 2024, 3, 10, 8),
            make_candle(Instrument::Ftse, 2024, 3, 10, 8),
        ];

        let written = store.write_candles(&candles).unwrap();
        assert_eq!(written, 2);

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 3, 31).unwrap(),
        )
        .unwrap();

        let dax = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(dax.len(), 1);

        let ftse = store.read_candles(Instrument::Ftse, range).unwrap();
        assert_eq!(ftse.len(), 1);
    }

    #[test]
    fn test_cross_month_range() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let candles = vec![
            make_candle(Instrument::Dax, 2024, 1, 31, 8),
            make_candle(Instrument::Dax, 2024, 2, 1, 8),
            make_candle(Instrument::Dax, 2024, 3, 1, 8),
        ];

        store.write_candles(&candles).unwrap();

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 1, 30).unwrap(),
            NaiveDate::from_ymd_opt(2024, 2, 2).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(read.len(), 2); // Jan 31 and Feb 1 only
    }

    #[test]
    fn test_parquet_path_format() {
        let path = parquet_path(Path::new("data"), Instrument::Dax, 2024, 1);
        assert_eq!(path, PathBuf::from("data/dax/2024-01.parquet"));
    }

    #[test]
    fn test_read_preserves_timestamp_order() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        // Write in reverse order.
        let candles = vec![
            make_candle(Instrument::Nasdaq, 2024, 5, 15, 16),
            make_candle(Instrument::Nasdaq, 2024, 5, 15, 14),
            make_candle(Instrument::Nasdaq, 2024, 5, 15, 10),
        ];

        store.write_candles(&candles).unwrap();

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 5, 31).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Nasdaq, range).unwrap();
        assert_eq!(read.len(), 3);
        // Should be sorted by timestamp.
        assert!(read[0].timestamp < read[1].timestamp);
        assert!(read[1].timestamp < read[2].timestamp);
    }

    #[test]
    fn test_write_100_candles_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let base = NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_utc();

        let candles: Vec<Candle> = (0..100)
            .map(|i| {
                let ts = base + chrono::Duration::minutes(15 * i);
                Candle {
                    instrument: Instrument::Dax,
                    timestamp: ts,
                    open: dec!(16000.50) + Decimal::from(i),
                    high: dec!(16050.00) + Decimal::from(i),
                    low: dec!(15980.25) + Decimal::from(i),
                    close: dec!(16030.75) + Decimal::from(i),
                    volume: 1000 + i,
                }
            })
            .collect();

        let written = store.write_candles(&candles).unwrap();
        assert_eq!(written, 100);

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(read.len(), 100);

        // Verify ordering.
        for pair in read.windows(2) {
            assert!(pair[0].timestamp <= pair[1].timestamp);
        }
    }

    #[test]
    fn test_parquet_path_format_all_instruments() {
        use std::path::Path;
        assert_eq!(
            parquet_path(Path::new("data"), Instrument::Dax, 2024, 1),
            PathBuf::from("data/dax/2024-01.parquet")
        );
        assert_eq!(
            parquet_path(Path::new("data"), Instrument::Ftse, 2024, 12),
            PathBuf::from("data/ftse/2024-12.parquet")
        );
        assert_eq!(
            parquet_path(Path::new("data"), Instrument::Nasdaq, 2025, 6),
            PathBuf::from("data/ixic/2025-06.parquet")
        );
        assert_eq!(
            parquet_path(Path::new("data"), Instrument::Dow, 2023, 3),
            PathBuf::from("data/dji/2023-03.parquet")
        );
    }

    #[test]
    fn test_overwrite_existing_file() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        // Write first batch.
        let candles1 = vec![make_candle(Instrument::Dax, 2024, 1, 15, 8)];
        store.write_candles(&candles1).unwrap();

        // Write second batch (same month, overwrites).
        let candles2 = vec![
            make_candle(Instrument::Dax, 2024, 1, 15, 9),
            make_candle(Instrument::Dax, 2024, 1, 15, 10),
        ];
        store.write_candles(&candles2).unwrap();

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        )
        .unwrap();

        // Should only have the second batch (overwrite behavior).
        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(read.len(), 2);
    }

    #[test]
    fn test_read_across_year_boundary() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let candles = vec![
            make_candle(Instrument::Dax, 2023, 12, 29, 8),
            make_candle(Instrument::Dax, 2023, 12, 30, 8),
            make_candle(Instrument::Dax, 2024, 1, 2, 8),
            make_candle(Instrument::Dax, 2024, 1, 3, 8),
        ];

        store.write_candles(&candles).unwrap();

        // Read across the year boundary.
        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2023, 12, 28).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 5).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(read.len(), 4);
        // First candle should be Dec 29, last should be Jan 3.
        assert_eq!(
            read[0].timestamp.date_naive(),
            NaiveDate::from_ymd_opt(2023, 12, 29).unwrap()
        );
        assert_eq!(
            read[3].timestamp.date_naive(),
            NaiveDate::from_ymd_opt(2024, 1, 3).unwrap()
        );
    }

    #[test]
    fn test_write_large_batch_2000_candles() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let base = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let candles: Vec<Candle> = (0..2000)
            .map(|i| {
                let ts = base + chrono::Duration::minutes(15 * i);
                Candle {
                    instrument: Instrument::Nasdaq,
                    timestamp: ts,
                    open: dec!(15000.00) + Decimal::from(i),
                    high: dec!(15100.00) + Decimal::from(i),
                    low: dec!(14900.00) + Decimal::from(i),
                    close: dec!(15050.00) + Decimal::from(i),
                    volume: i,
                }
            })
            .collect();

        let written = store.write_candles(&candles).unwrap();
        assert_eq!(written, 2000);

        // 2000 candles at 15-min = 500 hours = ~20.8 days, all in Jan 2024.
        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Nasdaq, range).unwrap();
        assert_eq!(read.len(), 2000);
    }

    #[test]
    fn test_read_single_day_range() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let candles = vec![
            make_candle(Instrument::Dax, 2024, 1, 14, 8),
            make_candle(Instrument::Dax, 2024, 1, 15, 8),
            make_candle(Instrument::Dax, 2024, 1, 15, 9),
            make_candle(Instrument::Dax, 2024, 1, 16, 8),
        ];

        store.write_candles(&candles).unwrap();

        // Read only Jan 15.
        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(read.len(), 2);
        for c in &read {
            assert_eq!(
                c.timestamp.date_naive(),
                NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()
            );
        }
    }

    #[test]
    fn test_all_four_instruments_independent_storage() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        for instrument in Instrument::ALL {
            let candle = Candle {
                instrument,
                timestamp: NaiveDate::from_ymd_opt(2024, 6, 1)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap()
                    .and_utc(),
                open: dec!(1000.0),
                high: dec!(1100.0),
                low: dec!(900.0),
                close: dec!(1050.0),
                volume: 999,
            };
            store.write_candles(&[candle]).unwrap();
        }

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
        )
        .unwrap();

        for instrument in Instrument::ALL {
            let read = store.read_candles(instrument, range).unwrap();
            assert_eq!(read.len(), 1, "Expected 1 candle for {instrument}");
            assert_eq!(read[0].instrument, instrument);
        }
    }

    #[test]
    fn test_decimal_precision_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let store = ParquetStore::new(tmp.path());

        let ts = NaiveDate::from_ymd_opt(2024, 6, 1)
            .unwrap()
            .and_hms_opt(9, 15, 0)
            .unwrap()
            .and_utc();

        let candle = Candle {
            instrument: Instrument::Dax,
            timestamp: ts,
            open: dec!(16123.45),
            high: dec!(16200.00),
            low: dec!(16100.10),
            close: dec!(16150.55),
            volume: 99999,
        };

        store.write_candles(&[candle]).unwrap();

        let range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
        )
        .unwrap();

        let read = store.read_candles(Instrument::Dax, range).unwrap();
        assert_eq!(read.len(), 1);
        // Float64 roundtrip may lose some precision, but should be close.
        let diff = (read[0].open - dec!(16123.45)).abs();
        assert!(diff < dec!(0.01));
    }
}
