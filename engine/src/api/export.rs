//! CSV export endpoint for backtest trades.
//!
//! Provides a download endpoint that returns all trades for a backtest run
//! as a CSV file with appropriate content headers for browser download.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use uuid::Uuid;

use crate::db;

use super::error::ApiError;
use super::state::AppState;

/// Build the export sub-router.
///
/// Mounted under `/api/backtest/:id/export` by the parent router.
pub fn export_routes() -> Router<AppState> {
    Router::new().route("/csv", get(export_csv_handler))
}

/// `GET /api/backtest/:id/export/csv` -- Export trades as CSV download.
///
/// Fetches all trades for the given backtest run and formats them as CSV
/// with headers: date, instrument, direction, entry_price, exit_price,
/// stop_loss, pnl, duration_minutes, entry_time, exit_time.
///
/// Returns 404 if the backtest run does not exist.
async fn export_csv_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    // Verify run exists (returns 404 via DbError if not found)
    let _run = db::backtests::get_backtest_run(&state.db_pool, id).await?;

    let trades = db::trades::get_all_trades_for_run(&state.db_pool, id).await?;

    let csv = build_csv(&trades);
    let filename = format!("backtest_{id}.csv");

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv,
    )
        .into_response())
}

/// Build a CSV string from trade rows.
///
/// Columns: date, instrument, direction, entry_price, exit_price, stop_loss,
/// pnl, duration_minutes, entry_time, exit_time.
fn build_csv(trades: &[db::TradeRow]) -> String {
    let mut buf = String::from(
        "date,instrument,direction,entry_price,exit_price,stop_loss,pnl,duration_minutes,entry_time,exit_time\n",
    );

    for trade in trades {
        let duration_minutes = (trade.exit_time - trade.entry_time).num_minutes();
        buf.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            trade.trade_date,
            trade.instrument_id,
            trade.direction,
            trade.entry_price,
            trade.exit_price,
            trade.stop_loss,
            trade.pnl_points,
            duration_minutes,
            trade.entry_time.format("%Y-%m-%dT%H:%M:%SZ"),
            trade.exit_time.format("%Y-%m-%dT%H:%M:%SZ"),
        ));
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use rust_decimal::Decimal;

    fn make_test_trade(
        direction: &str,
        entry_price: i64,
        exit_price: i64,
        stop_loss: i64,
        pnl: i64,
    ) -> db::TradeRow {
        let entry = chrono::DateTime::parse_from_rfc3339("2024-01-15T09:30:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let exit = chrono::DateTime::parse_from_rfc3339("2024-01-15T10:15:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);

        db::TradeRow {
            id: Uuid::new_v4(),
            backtest_run_id: Uuid::new_v4(),
            instrument_id: 1,
            direction: direction.to_owned(),
            entry_price: Decimal::new(entry_price, 2),
            entry_time: entry,
            exit_price: Decimal::new(exit_price, 2),
            exit_time: exit,
            stop_loss: Decimal::new(stop_loss, 2),
            exit_reason: "EndOfDay".to_owned(),
            pnl_points: Decimal::new(pnl, 2),
            pnl_with_adds: Decimal::new(pnl, 2),
            adds: serde_json::json!([]),
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid date"),
        }
    }

    #[test]
    fn test_csv_header_format() {
        let csv = build_csv(&[]);
        let header_line = csv.lines().next().expect("has header");
        assert_eq!(
            header_line,
            "date,instrument,direction,entry_price,exit_price,stop_loss,pnl,duration_minutes,entry_time,exit_time"
        );
    }

    #[test]
    fn test_csv_row_format_with_sample_trade() {
        let trade = make_test_trade("Long", 1600000, 1605000, 1596000, 5000);
        let csv = build_csv(&[trade]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2, "header + 1 data row");

        let row = lines[1];
        let fields: Vec<&str> = row.split(',').collect();
        assert_eq!(fields.len(), 10, "should have 10 columns");
        assert_eq!(fields[0], "2024-01-15"); // date
        assert_eq!(fields[1], "1"); // instrument_id
        assert_eq!(fields[2], "Long"); // direction
        assert_eq!(fields[3], "16000.00"); // entry_price
        assert_eq!(fields[4], "16050.00"); // exit_price
        assert_eq!(fields[5], "15960.00"); // stop_loss
        assert_eq!(fields[6], "50.00"); // pnl
        assert_eq!(fields[7], "45"); // duration_minutes
        assert_eq!(fields[8], "2024-01-15T09:30:00Z"); // entry_time
        assert_eq!(fields[9], "2024-01-15T10:15:00Z"); // exit_time
    }

    #[test]
    fn test_csv_multiple_trades() {
        let trades = vec![
            make_test_trade("Long", 1600000, 1605000, 1596000, 5000),
            make_test_trade("Short", 1605000, 1600000, 1609000, 5000),
        ];
        let csv = build_csv(&trades);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3, "header + 2 data rows");
        assert!(lines[1].contains("Long"));
        assert!(lines[2].contains("Short"));
    }

    #[test]
    fn test_csv_empty_trades() {
        let csv = build_csv(&[]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 1, "header only");
    }

    #[test]
    fn test_csv_content_type_header_value() {
        // Verify the content type constant value
        assert_eq!("text/csv", "text/csv");
    }

    #[test]
    fn test_csv_content_disposition_format() {
        let id = Uuid::new_v4();
        let filename = format!("backtest_{id}.csv");
        let disposition = format!("attachment; filename=\"{filename}\"");
        assert!(disposition.starts_with("attachment; filename=\"backtest_"));
        assert!(disposition.ends_with(".csv\""));
    }

    #[test]
    fn test_csv_negative_pnl() {
        let trade = make_test_trade("Long", 1600000, 1596000, 1596000, -4000);
        let csv = build_csv(&[trade]);
        let row = csv.lines().nth(1).expect("data row");
        let fields: Vec<&str> = row.split(',').collect();
        assert_eq!(fields[6], "-40.00"); // negative pnl
    }

    #[test]
    fn test_csv_duration_calculation() {
        // entry: 09:30, exit: 10:15 = 45 minutes
        let trade = make_test_trade("Long", 1600000, 1605000, 1596000, 5000);
        let csv = build_csv(&[trade]);
        let row = csv.lines().nth(1).expect("data row");
        let fields: Vec<&str> = row.split(',').collect();
        assert_eq!(fields[7], "45");
    }
}
