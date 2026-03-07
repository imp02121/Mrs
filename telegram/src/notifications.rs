//! Message formatting and delivery for Telegram notifications.
//!
//! Provides templated message builders for each signal event type
//! (signal bar formed, order triggered, trade closed, daily summary)
//! and a helper to deliver messages via the Telegram Bot API.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use teloxide::prelude::*;
use teloxide::types::ChatId;

use crate::signals::SignalData;

/// Format a number with thousand separators (comma-separated).
///
/// Handles both integer-like decimals and those with fractional parts.
fn format_number(d: &Decimal) -> String {
    let s = d.to_string();
    let (integer_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s.as_str(), None),
    };

    let negative = integer_part.starts_with('-');
    let digits = if negative {
        &integer_part[1..]
    } else {
        integer_part
    };

    let mut result = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    let formatted: String = result.chars().rev().collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&formatted);
    if let Some(frac) = frac_part {
        out.push('.');
        out.push_str(frac);
    }
    out
}

/// Map an instrument database ID to its display name.
///
/// Returns `"Unknown"` for unrecognized IDs.
#[must_use]
pub fn instrument_name(instrument_id: i16) -> &'static str {
    match instrument_id {
        1 => "DAX",
        2 => "FTSE",
        3 => "NASDAQ",
        4 => "DOW",
        _ => "Unknown",
    }
}

/// Map an instrument name to its signal bar time window string.
///
/// Uses the exchange-local times for the 2nd 15-minute candle.
#[must_use]
pub fn signal_bar_time_window(instrument_id: i16) -> &'static str {
    match instrument_id {
        1 => "09:15 - 09:30 CET",
        2 => "08:15 - 08:30 GMT",
        3 => "09:45 - 10:00 ET",
        4 => "09:45 - 10:00 ET",
        _ => "Unknown",
    }
}

/// Format a "Signal Bar Formed" notification message.
#[must_use]
pub fn format_signal_bar_formed(signal: &SignalData) -> String {
    let name = instrument_name(signal.instrument_id);
    let time_window = signal_bar_time_window(signal.instrument_id);
    let high = format_number(&signal.signal_bar_high);
    let low = format_number(&signal.signal_bar_low);
    let buy = format_number(&signal.buy_level);
    let sell = format_number(&signal.sell_level);

    format!(
        "\u{1f3af} {name} Signal Bar Formed\n\
         Time: {time_window}\n\
         High: {high} | Low: {low}\n\
         Buy above: {buy} | Sell below: {sell}"
    )
}

/// Format an "Order Triggered" notification message.
///
/// `direction` should be `"LONG"` or `"SHORT"`.
#[must_use]
pub fn format_order_triggered(
    signal: &SignalData,
    direction: &str,
    entry: &Decimal,
    stop_loss: &Decimal,
) -> String {
    let name = instrument_name(signal.instrument_id);
    let entry_str = format_number(entry);
    let sl_str = format_number(stop_loss);
    let risk = (*entry - *stop_loss).abs();
    let risk_str = format_number(&risk);

    format!(
        "\u{26a1} {name} {direction} Triggered\n\
         Entry: {entry_str} | SL: {sl_str} | Risk: {risk_str} pts"
    )
}

/// Format a "Trade Closed" notification message.
#[must_use]
pub fn format_trade_closed(
    instrument_id: i16,
    direction: &str,
    entry: &Decimal,
    exit: &Decimal,
    pnl_points: &Decimal,
    duration: &str,
) -> String {
    let name = instrument_name(instrument_id);
    let entry_str = format_number(entry);
    let exit_str = format_number(exit);
    let sign = if *pnl_points >= Decimal::ZERO {
        "+"
    } else {
        ""
    };
    let pnl_str = format_number(pnl_points);

    format!(
        "\u{2705} {name} {direction} Closed\n\
         Entry: {entry_str} \u{2192} Exit: {exit_str}\n\
         PnL: {sign}{pnl_str} points | Duration: {duration}"
    )
}

/// A single instrument's result in a daily summary.
pub struct DailyInstrumentResult {
    /// Instrument name (e.g. "DAX").
    pub name: String,
    /// Trade direction (e.g. "LONG", "SHORT", or "NO TRADE").
    pub direction: String,
    /// Points gained or lost.
    pub pnl_points: Decimal,
}

/// Format a "Daily Summary" notification message.
#[must_use]
pub fn format_daily_summary(date: NaiveDate, results: &[DailyInstrumentResult]) -> String {
    let date_str = date.format("%-d %b %Y").to_string();
    let mut lines = vec![format!("\u{1f4ca} Daily Summary - {date_str}")];

    for r in results {
        let sign = if r.pnl_points >= Decimal::ZERO {
            "+"
        } else {
            ""
        };
        let pnl = format_number(&r.pnl_points);
        lines.push(format!(
            "{:<6}{:<6}{}{} pts",
            r.name, r.direction, sign, pnl
        ));
    }

    let total: Decimal = results.iter().map(|r| r.pnl_points).sum();
    let total_sign = if total >= Decimal::ZERO { "+" } else { "" };
    let total_str = format_number(&total);
    lines.push(format!("Day Total: {total_sign}{total_str} points"));

    lines.join("\n")
}

/// Send a text message to a specific Telegram chat.
///
/// # Errors
///
/// Returns a `teloxide::RequestError` if the message cannot be delivered.
pub async fn send_notification(
    bot: &Bot,
    chat_id: i64,
    message: &str,
) -> Result<(), teloxide::RequestError> {
    bot.send_message(ChatId(chat_id), message).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn sample_signal() -> SignalData {
        SignalData {
            id: Uuid::nil().to_string(),
            instrument_id: 1,
            signal_date: "2024-06-15".into(),
            signal_bar_high: dec!(22448.00),
            signal_bar_low: dec!(22390.00),
            buy_level: dec!(22450.00),
            sell_level: dec!(22388.00),
            status: "pending".into(),
            fill_details: None,
            created_at: String::new(),
        }
    }

    #[test]
    fn test_format_number_with_thousands() {
        assert_eq!(format_number(&dec!(22448.00)), "22,448.00");
        assert_eq!(format_number(&dec!(1000)), "1,000");
        assert_eq!(format_number(&dec!(999)), "999");
        assert_eq!(format_number(&dec!(1234567)), "1,234,567");
    }

    #[test]
    fn test_format_number_negative() {
        assert_eq!(format_number(&dec!(-25.00)), "-25.00");
        assert_eq!(format_number(&dec!(-1234)), "-1,234");
    }

    #[test]
    fn test_format_number_zero() {
        assert_eq!(format_number(&dec!(0)), "0");
    }

    #[test]
    fn test_instrument_name_known() {
        assert_eq!(instrument_name(1), "DAX");
        assert_eq!(instrument_name(2), "FTSE");
        assert_eq!(instrument_name(3), "NASDAQ");
        assert_eq!(instrument_name(4), "DOW");
    }

    #[test]
    fn test_instrument_name_unknown() {
        assert_eq!(instrument_name(99), "Unknown");
    }

    #[test]
    fn test_signal_bar_time_window() {
        assert_eq!(signal_bar_time_window(1), "09:15 - 09:30 CET");
        assert_eq!(signal_bar_time_window(2), "08:15 - 08:30 GMT");
        assert_eq!(signal_bar_time_window(3), "09:45 - 10:00 ET");
        assert_eq!(signal_bar_time_window(4), "09:45 - 10:00 ET");
    }

    #[test]
    fn test_format_signal_bar_formed() {
        let signal = sample_signal();
        let msg = format_signal_bar_formed(&signal);
        assert!(msg.contains("DAX Signal Bar Formed"));
        assert!(msg.contains("09:15 - 09:30 CET"));
        assert!(msg.contains("22,448.00"));
        assert!(msg.contains("22,390.00"));
        assert!(msg.contains("Buy above: 22,450.00"));
        assert!(msg.contains("Sell below: 22,388.00"));
    }

    #[test]
    fn test_format_order_triggered() {
        let signal = sample_signal();
        let entry = dec!(22450.00);
        let stop_loss = dec!(22390.00);
        let msg = format_order_triggered(&signal, "LONG", &entry, &stop_loss);
        assert!(msg.contains("DAX LONG Triggered"));
        assert!(msg.contains("22,450.00"));
        assert!(msg.contains("SL: 22,390.00"));
        assert!(msg.contains("Risk: 60.00 pts"));
    }

    #[test]
    fn test_format_trade_closed_profit() {
        let msg = format_trade_closed(
            1,
            "LONG",
            &dec!(22450.00),
            &dec!(22510.00),
            &dec!(60.00),
            "4h 13m",
        );
        assert!(msg.contains("DAX LONG Closed"));
        assert!(msg.contains("+60.00 points"));
        assert!(msg.contains("4h 13m"));
    }

    #[test]
    fn test_format_trade_closed_loss() {
        let msg = format_trade_closed(
            2,
            "SHORT",
            &dec!(7500.00),
            &dec!(7525.00),
            &dec!(-25.00),
            "2h 5m",
        );
        assert!(msg.contains("FTSE SHORT Closed"));
        assert!(msg.contains("-25.00 points"));
    }

    #[test]
    fn test_format_daily_summary() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 6).expect("valid date");
        let results = vec![
            DailyInstrumentResult {
                name: "DAX".into(),
                direction: "LONG".into(),
                pnl_points: dec!(60),
            },
            DailyInstrumentResult {
                name: "FTSE".into(),
                direction: "SHORT".into(),
                pnl_points: dec!(-25),
            },
        ];
        let msg = format_daily_summary(date, &results);
        assert!(msg.contains("Daily Summary - 6 Mar 2026"));
        assert!(msg.contains("DAX"));
        assert!(msg.contains("+60"));
        assert!(msg.contains("-25"));
        assert!(msg.contains("Day Total: +35 points"));
    }

    #[test]
    fn test_format_daily_summary_empty() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 7).expect("valid date");
        let msg = format_daily_summary(date, &[]);
        assert!(msg.contains("Daily Summary"));
        assert!(msg.contains("Day Total: +0 points"));
    }

    // ---- format_number edge cases ----

    #[test]
    fn test_format_number_very_large() {
        assert_eq!(format_number(&dec!(100000)), "100,000");
        assert_eq!(format_number(&dec!(999999)), "999,999");
        assert_eq!(format_number(&dec!(1000000)), "1,000,000");
        assert_eq!(format_number(&dec!(123456789)), "123,456,789");
    }

    #[test]
    fn test_format_number_decimal_precision() {
        assert_eq!(format_number(&dec!(1.5)), "1.5");
        assert_eq!(format_number(&dec!(1234.567)), "1,234.567");
        assert_eq!(format_number(&dec!(0.01)), "0.01");
        assert_eq!(format_number(&dec!(100000.99)), "100,000.99");
    }

    #[test]
    fn test_format_number_single_digit() {
        assert_eq!(format_number(&dec!(1)), "1");
        assert_eq!(format_number(&dec!(9)), "9");
    }

    #[test]
    fn test_format_number_negative_large() {
        assert_eq!(format_number(&dec!(-100000)), "-100,000");
        assert_eq!(format_number(&dec!(-1234567.89)), "-1,234,567.89");
    }

    // ---- Signal bar formed for different instruments ----

    #[test]
    fn test_format_signal_formed_ftse() {
        let signal = SignalData {
            id: Uuid::nil().to_string(),
            instrument_id: 2,
            signal_date: "2024-06-15".into(),
            signal_bar_high: dec!(7650.00),
            signal_bar_low: dec!(7620.00),
            buy_level: dec!(7652.00),
            sell_level: dec!(7618.00),
            status: "pending".into(),
            fill_details: None,
            created_at: String::new(),
        };
        let msg = format_signal_bar_formed(&signal);
        assert!(msg.contains("FTSE Signal Bar Formed"));
        assert!(msg.contains("08:15 - 08:30 GMT"));
        assert!(msg.contains("7,650.00"));
        assert!(msg.contains("7,620.00"));
    }

    #[test]
    fn test_format_signal_formed_nasdaq() {
        let signal = SignalData {
            id: Uuid::nil().to_string(),
            instrument_id: 3,
            signal_date: "2024-06-15".into(),
            signal_bar_high: dec!(18250.50),
            signal_bar_low: dec!(18190.25),
            buy_level: dec!(18252.50),
            sell_level: dec!(18188.25),
            status: "pending".into(),
            fill_details: None,
            created_at: String::new(),
        };
        let msg = format_signal_bar_formed(&signal);
        assert!(msg.contains("NASDAQ Signal Bar Formed"));
        assert!(msg.contains("09:45 - 10:00 ET"));
        assert!(msg.contains("18,250.50"));
    }

    #[test]
    fn test_format_signal_formed_dow() {
        let signal = SignalData {
            id: Uuid::nil().to_string(),
            instrument_id: 4,
            signal_date: "2024-06-15".into(),
            signal_bar_high: dec!(39500.00),
            signal_bar_low: dec!(39450.00),
            buy_level: dec!(39502.00),
            sell_level: dec!(39448.00),
            status: "pending".into(),
            fill_details: None,
            created_at: String::new(),
        };
        let msg = format_signal_bar_formed(&signal);
        assert!(msg.contains("DOW Signal Bar Formed"));
        assert!(msg.contains("09:45 - 10:00 ET"));
        assert!(msg.contains("39,500.00"));
    }

    #[test]
    fn test_format_signal_formed_unknown_instrument() {
        let signal = SignalData {
            id: Uuid::nil().to_string(),
            instrument_id: 99,
            signal_date: "2024-06-15".into(),
            signal_bar_high: dec!(100.00),
            signal_bar_low: dec!(90.00),
            buy_level: dec!(102.00),
            sell_level: dec!(88.00),
            status: "pending".into(),
            fill_details: None,
            created_at: String::new(),
        };
        let msg = format_signal_bar_formed(&signal);
        assert!(msg.contains("Unknown Signal Bar Formed"));
        assert!(msg.contains("Unknown")); // time window
    }

    // ---- Emoji presence ----

    #[test]
    fn test_format_signal_bar_formed_contains_emoji() {
        let signal = sample_signal();
        let msg = format_signal_bar_formed(&signal);
        assert!(msg.contains('\u{1f3af}')); // target emoji
    }

    #[test]
    fn test_format_order_triggered_contains_emoji() {
        let signal = sample_signal();
        let msg = format_order_triggered(&signal, "LONG", &dec!(22450.00), &dec!(22390.00));
        assert!(msg.contains('\u{26a1}')); // lightning bolt emoji
    }

    #[test]
    fn test_format_trade_closed_contains_emoji() {
        let msg = format_trade_closed(
            1,
            "LONG",
            &dec!(22450.00),
            &dec!(22510.00),
            &dec!(60.00),
            "4h 13m",
        );
        assert!(msg.contains('\u{2705}')); // check mark emoji
        assert!(msg.contains('\u{2192}')); // right arrow
    }

    #[test]
    fn test_format_daily_summary_contains_emoji() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 7).expect("valid date");
        let msg = format_daily_summary(date, &[]);
        assert!(msg.contains('\u{1f4ca}')); // chart emoji
    }

    // ---- Order triggered edge cases ----

    #[test]
    fn test_format_order_triggered_short() {
        let signal = sample_signal();
        let entry = dec!(22388.00);
        let stop_loss = dec!(22448.00);
        let msg = format_order_triggered(&signal, "SHORT", &entry, &stop_loss);
        assert!(msg.contains("DAX SHORT Triggered"));
        assert!(msg.contains("Risk: 60.00 pts"));
    }

    #[test]
    fn test_format_order_triggered_zero_risk() {
        let signal = sample_signal();
        let entry = dec!(22450.00);
        let stop_loss = dec!(22450.00);
        let msg = format_order_triggered(&signal, "LONG", &entry, &stop_loss);
        assert!(msg.contains("Risk: 0.00 pts"));
    }

    // ---- Trade closed edge cases ----

    #[test]
    fn test_format_trade_closed_zero_pnl() {
        let msg = format_trade_closed(
            3,
            "LONG",
            &dec!(18000.00),
            &dec!(18000.00),
            &dec!(0.00),
            "0h 5m",
        );
        assert!(msg.contains("NASDAQ LONG Closed"));
        assert!(msg.contains("+0.00 points"));
    }

    #[test]
    fn test_format_trade_closed_large_numbers() {
        let msg = format_trade_closed(
            4,
            "SHORT",
            &dec!(39500.00),
            &dec!(39350.00),
            &dec!(150.00),
            "6h 30m",
        );
        assert!(msg.contains("DOW SHORT Closed"));
        assert!(msg.contains("Entry: 39,500.00"));
        assert!(msg.contains("Exit: 39,350.00"));
        assert!(msg.contains("+150.00 points"));
    }

    // ---- Daily summary edge cases ----

    #[test]
    fn test_format_daily_summary_single_instrument() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 7).expect("valid date");
        let results = vec![DailyInstrumentResult {
            name: "DAX".into(),
            direction: "LONG".into(),
            pnl_points: dec!(42.5),
        }];
        let msg = format_daily_summary(date, &results);
        assert!(msg.contains("Daily Summary - 7 Mar 2026"));
        assert!(msg.contains("DAX"));
        assert!(msg.contains("+42.5"));
        assert!(msg.contains("Day Total: +42.5 points"));
    }

    #[test]
    fn test_format_daily_summary_all_losses() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15).expect("valid date");
        let results = vec![
            DailyInstrumentResult {
                name: "DAX".into(),
                direction: "SHORT".into(),
                pnl_points: dec!(-30),
            },
            DailyInstrumentResult {
                name: "FTSE".into(),
                direction: "LONG".into(),
                pnl_points: dec!(-15),
            },
        ];
        let msg = format_daily_summary(date, &results);
        assert!(msg.contains("Day Total: -45 points"));
    }

    #[test]
    fn test_format_daily_summary_no_trade_entry() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 2, 10).expect("valid date");
        let results = vec![
            DailyInstrumentResult {
                name: "DAX".into(),
                direction: "NO TRADE".into(),
                pnl_points: dec!(0),
            },
            DailyInstrumentResult {
                name: "FTSE".into(),
                direction: "LONG".into(),
                pnl_points: dec!(20),
            },
        ];
        let msg = format_daily_summary(date, &results);
        assert!(msg.contains("NO TRADE"));
        assert!(msg.contains("Day Total: +20 points"));
    }

    // ---- instrument_name and signal_bar_time_window boundary ----

    #[test]
    fn test_instrument_name_zero() {
        assert_eq!(instrument_name(0), "Unknown");
    }

    #[test]
    fn test_instrument_name_negative() {
        assert_eq!(instrument_name(-1), "Unknown");
    }

    #[test]
    fn test_signal_bar_time_window_unknown() {
        assert_eq!(signal_bar_time_window(0), "Unknown");
        assert_eq!(signal_bar_time_window(5), "Unknown");
        assert_eq!(signal_bar_time_window(-1), "Unknown");
    }
}
