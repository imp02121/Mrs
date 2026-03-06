//! Trading instruments supported by the School Run Strategy.
//!
//! Each instrument carries metadata about its exchange session: timezone,
//! market open/close times, and signal bar timing. The critical
//! [`Instrument::signal_bar_start_utc`] method performs DST-aware conversion
//! so that the rest of the engine can work exclusively in UTC.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

/// Error returned when parsing an unknown instrument string.
#[derive(Debug, thiserror::Error)]
#[error("unknown instrument: \"{0}\"")]
pub struct ParseInstrumentError(String);

/// A supported trading instrument.
///
/// Each variant represents an equity index tracked by the School Run Strategy.
/// Instruments determine session times, timezone rules, and signal bar
/// positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Instrument {
    /// DAX 40 index (XETRA, Frankfurt).
    Dax,
    /// FTSE 100 index (LSE, London).
    Ftse,
    /// Nasdaq Composite (NYSE/NASDAQ, New York).
    Nasdaq,
    /// Dow Jones Industrial Average (NYSE, New York).
    Dow,
}

impl Instrument {
    /// All supported instruments, in the order they appear in the enum.
    pub const ALL: [Instrument; 4] = [
        Instrument::Dax,
        Instrument::Ftse,
        Instrument::Nasdaq,
        Instrument::Dow,
    ];

    /// Ticker symbol used by data-provider APIs (e.g. Twelve Data).
    ///
    /// Returns: `"DAX"`, `"FTSE"`, `"IXIC"`, or `"DJI"`.
    #[must_use]
    pub fn ticker(self) -> &'static str {
        match self {
            Self::Dax => "DAX",
            Self::Ftse => "FTSE",
            Self::Nasdaq => "IXIC",
            Self::Dow => "DJI",
        }
    }

    /// Human-readable display name.
    ///
    /// Returns: `"DAX 40"`, `"FTSE 100"`, `"Nasdaq Composite"`, or
    /// `"Dow Jones"`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Dax => "DAX 40",
            Self::Ftse => "FTSE 100",
            Self::Nasdaq => "Nasdaq Composite",
            Self::Dow => "Dow Jones",
        }
    }

    /// Exchange timezone as a `chrono_tz::Tz` value.
    ///
    /// - DAX: `Europe/Berlin` (CET/CEST)
    /// - FTSE: `Europe/London` (GMT/BST)
    /// - Nasdaq / Dow: `America/New_York` (EST/EDT)
    #[must_use]
    pub fn exchange_timezone(self) -> Tz {
        match self {
            Self::Dax => chrono_tz::Europe::Berlin,
            Self::Ftse => chrono_tz::Europe::London,
            Self::Nasdaq | Self::Dow => chrono_tz::America::New_York,
        }
    }

    /// Market open time in the exchange's local timezone.
    ///
    /// - DAX: 09:00
    /// - FTSE: 08:00
    /// - Nasdaq / Dow: 09:30
    #[must_use]
    pub fn market_open_local(self) -> NaiveTime {
        match self {
            Self::Dax => NaiveTime::from_hms_opt(9, 0, 0),
            Self::Ftse => NaiveTime::from_hms_opt(8, 0, 0),
            Self::Nasdaq | Self::Dow => NaiveTime::from_hms_opt(9, 30, 0),
        }
        .expect("hardcoded valid time")
    }

    /// Signal bar start time in the exchange's local timezone.
    ///
    /// The signal bar is the 2nd 15-minute candle after market open:
    /// - DAX: 09:15 (09:00 + 15 min)
    /// - FTSE: 08:15 (08:00 + 15 min)
    /// - Nasdaq / Dow: 09:45 (09:30 + 15 min)
    #[must_use]
    pub fn signal_bar_start_local(self) -> NaiveTime {
        match self {
            Self::Dax => NaiveTime::from_hms_opt(9, 15, 0),
            Self::Ftse => NaiveTime::from_hms_opt(8, 15, 0),
            Self::Nasdaq | Self::Dow => NaiveTime::from_hms_opt(9, 45, 0),
        }
        .expect("hardcoded valid time")
    }

    /// Market close time in the exchange's local timezone.
    ///
    /// - DAX: 17:30
    /// - FTSE: 16:30
    /// - Nasdaq / Dow: 16:00
    #[must_use]
    pub fn market_close_local(self) -> NaiveTime {
        match self {
            Self::Dax => NaiveTime::from_hms_opt(17, 30, 0),
            Self::Ftse => NaiveTime::from_hms_opt(16, 30, 0),
            Self::Nasdaq | Self::Dow => NaiveTime::from_hms_opt(16, 0, 0),
        }
        .expect("hardcoded valid time")
    }

    /// Signal bar start time converted to UTC for a specific date.
    ///
    /// This is the **critical** DST-aware conversion. The local signal-bar
    /// time is fixed (e.g. 09:15 CET for DAX), but the UTC equivalent shifts
    /// when clocks change:
    ///
    /// - DAX winter (CET, UTC+1): 09:15 local = 08:15 UTC
    /// - DAX summer (CEST, UTC+2): 09:15 local = 07:15 UTC
    /// - FTSE winter (GMT, UTC+0): 08:15 local = 08:15 UTC
    /// - FTSE summer (BST, UTC+1): 08:15 local = 07:15 UTC
    /// - US winter (EST, UTC-5): 09:45 local = 14:45 UTC
    /// - US summer (EDT, UTC-4): 09:45 local = 13:45 UTC
    ///
    /// # Errors
    ///
    /// Returns `None` if the local time is ambiguous or non-existent on
    /// the given date (e.g. during the spring-forward gap). In practice
    /// market open times never fall inside DST transition gaps, so this
    /// should not occur for the instruments we support.
    #[must_use]
    pub fn signal_bar_start_utc(self, date: NaiveDate) -> Option<DateTime<Utc>> {
        let tz = self.exchange_timezone();
        let local_time = self.signal_bar_start_local();
        let naive_dt = date.and_time(local_time);
        match tz.from_local_datetime(&naive_dt) {
            chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
            chrono::LocalResult::Ambiguous(earliest, _) => Some(earliest.with_timezone(&Utc)),
            chrono::LocalResult::None => None,
        }
    }
}

impl fmt::Display for Instrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Instrument {
    type Err = ParseInstrumentError;

    /// Parse an instrument from a case-insensitive string.
    ///
    /// Accepts ticker symbols (`"DAX"`, `"FTSE"`, `"IXIC"`, `"DJI"`),
    /// variant names (`"dax"`, `"nasdaq"`), and common aliases.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "DAX" | "DAX40" | "DAX 40" => Ok(Self::Dax),
            "FTSE" | "FTSE100" | "FTSE 100" | "UKX" => Ok(Self::Ftse),
            "NASDAQ" | "IXIC" | "NDX" | "NQ" => Ok(Self::Nasdaq),
            "DOW" | "DJI" | "DJIA" => Ok(Self::Dow),
            _ => Err(ParseInstrumentError(s.to_owned())),
        }
    }
}

impl TryFrom<&str> for Instrument {
    type Error = ParseInstrumentError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_ticker_symbols() {
        assert_eq!(Instrument::Dax.ticker(), "DAX");
        assert_eq!(Instrument::Ftse.ticker(), "FTSE");
        assert_eq!(Instrument::Nasdaq.ticker(), "IXIC");
        assert_eq!(Instrument::Dow.ticker(), "DJI");
    }

    #[test]
    fn test_display_names() {
        assert_eq!(Instrument::Dax.name(), "DAX 40");
        assert_eq!(Instrument::Ftse.name(), "FTSE 100");
        assert_eq!(Instrument::Nasdaq.name(), "Nasdaq Composite");
        assert_eq!(Instrument::Dow.name(), "Dow Jones");
    }

    #[test]
    fn test_display_trait() {
        assert_eq!(format!("{}", Instrument::Dax), "DAX 40");
    }

    #[test]
    fn test_from_str_ticker() {
        assert_eq!("DAX".parse::<Instrument>().unwrap(), Instrument::Dax);
        assert_eq!("FTSE".parse::<Instrument>().unwrap(), Instrument::Ftse);
        assert_eq!("IXIC".parse::<Instrument>().unwrap(), Instrument::Nasdaq);
        assert_eq!("DJI".parse::<Instrument>().unwrap(), Instrument::Dow);
    }

    #[test]
    fn test_from_str_case_insensitive() {
        assert_eq!("dax".parse::<Instrument>().unwrap(), Instrument::Dax);
        assert_eq!("nasdaq".parse::<Instrument>().unwrap(), Instrument::Nasdaq);
        assert_eq!("dow".parse::<Instrument>().unwrap(), Instrument::Dow);
    }

    #[test]
    fn test_from_str_aliases() {
        assert_eq!("NQ".parse::<Instrument>().unwrap(), Instrument::Nasdaq);
        assert_eq!("DJIA".parse::<Instrument>().unwrap(), Instrument::Dow);
        assert_eq!("UKX".parse::<Instrument>().unwrap(), Instrument::Ftse);
    }

    #[test]
    fn test_from_str_unknown() {
        let err = "UNKNOWN".parse::<Instrument>().unwrap_err();
        assert_eq!(err.to_string(), "unknown instrument: \"UNKNOWN\"");
    }

    #[test]
    fn test_try_from_str() {
        assert_eq!(Instrument::try_from("DAX").unwrap(), Instrument::Dax);
        assert!(Instrument::try_from("NOPE").is_err());
    }

    // -- DST-aware signal_bar_start_utc tests --

    #[test]
    fn test_dax_signal_bar_utc_winter() {
        // 2024-01-15 is in CET (UTC+1). 09:15 CET = 08:15 UTC.
        let dt = Instrument::Dax
            .signal_bar_start_utc(date(2024, 1, 15))
            .unwrap();
        assert_eq!(dt.hour(), 8);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_dax_signal_bar_utc_summer() {
        // 2024-07-15 is in CEST (UTC+2). 09:15 CEST = 07:15 UTC.
        let dt = Instrument::Dax
            .signal_bar_start_utc(date(2024, 7, 15))
            .unwrap();
        assert_eq!(dt.hour(), 7);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_ftse_signal_bar_utc_winter() {
        // 2024-01-15 is in GMT (UTC+0). 08:15 GMT = 08:15 UTC.
        let dt = Instrument::Ftse
            .signal_bar_start_utc(date(2024, 1, 15))
            .unwrap();
        assert_eq!(dt.hour(), 8);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_ftse_signal_bar_utc_summer() {
        // 2024-07-15 is in BST (UTC+1). 08:15 BST = 07:15 UTC.
        let dt = Instrument::Ftse
            .signal_bar_start_utc(date(2024, 7, 15))
            .unwrap();
        assert_eq!(dt.hour(), 7);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_nasdaq_signal_bar_utc_winter() {
        // 2024-01-15 is in EST (UTC-5). 09:45 EST = 14:45 UTC.
        let dt = Instrument::Nasdaq
            .signal_bar_start_utc(date(2024, 1, 15))
            .unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_nasdaq_signal_bar_utc_summer() {
        // 2024-07-15 is in EDT (UTC-4). 09:45 EDT = 13:45 UTC.
        let dt = Instrument::Nasdaq
            .signal_bar_start_utc(date(2024, 7, 15))
            .unwrap();
        assert_eq!(dt.hour(), 13);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_dow_signal_bar_utc_same_as_nasdaq() {
        // Dow uses the same timezone and signal bar time as Nasdaq.
        let dt_nasdaq = Instrument::Nasdaq
            .signal_bar_start_utc(date(2024, 3, 20))
            .unwrap();
        let dt_dow = Instrument::Dow
            .signal_bar_start_utc(date(2024, 3, 20))
            .unwrap();
        assert_eq!(dt_nasdaq, dt_dow);
    }

    #[test]
    fn test_dax_dst_transition_spring_2024() {
        // Europe/Berlin DST transition: 2024-03-31 at 02:00 -> 03:00.
        // 09:15 on that date is already in CEST (UTC+2) = 07:15 UTC.
        let dt = Instrument::Dax
            .signal_bar_start_utc(date(2024, 3, 31))
            .unwrap();
        assert_eq!(dt.hour(), 7);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_dax_dst_transition_autumn_2024() {
        // Europe/Berlin DST transition: 2024-10-27 at 03:00 -> 02:00.
        // 09:15 on that date is in CET (UTC+1) = 08:15 UTC.
        let dt = Instrument::Dax
            .signal_bar_start_utc(date(2024, 10, 27))
            .unwrap();
        assert_eq!(dt.hour(), 8);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_ftse_dst_transition_spring_2024() {
        // Europe/London DST transition: 2024-03-31 at 01:00 -> 02:00.
        // 08:15 on that date is in BST (UTC+1) = 07:15 UTC.
        let dt = Instrument::Ftse
            .signal_bar_start_utc(date(2024, 3, 31))
            .unwrap();
        assert_eq!(dt.hour(), 7);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_us_dst_transition_spring_2024() {
        // America/New_York DST transition: 2024-03-10 at 02:00 -> 03:00.
        // 09:45 on that date is in EDT (UTC-4) = 13:45 UTC.
        let dt = Instrument::Nasdaq
            .signal_bar_start_utc(date(2024, 3, 10))
            .unwrap();
        assert_eq!(dt.hour(), 13);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_dow_signal_bar_utc_winter() {
        // 2024-01-15 is in EST (UTC-5). 09:45 EST = 14:45 UTC.
        let dt = Instrument::Dow
            .signal_bar_start_utc(date(2024, 1, 15))
            .unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_dow_signal_bar_utc_summer() {
        // 2024-07-15 is in EDT (UTC-4). 09:45 EDT = 13:45 UTC.
        let dt = Instrument::Dow
            .signal_bar_start_utc(date(2024, 7, 15))
            .unwrap();
        assert_eq!(dt.hour(), 13);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_ftse_dst_transition_autumn_2024() {
        // Europe/London DST transition: 2024-10-27 at 02:00 -> 01:00.
        // 08:15 on that date is in GMT (UTC+0) = 08:15 UTC.
        let dt = Instrument::Ftse
            .signal_bar_start_utc(date(2024, 10, 27))
            .unwrap();
        assert_eq!(dt.hour(), 8);
        assert_eq!(dt.minute(), 15);
    }

    #[test]
    fn test_us_dst_transition_autumn_2024() {
        // America/New_York DST transition: 2024-11-03 at 02:00 -> 01:00.
        // 09:45 on that date is in EST (UTC-5) = 14:45 UTC.
        let dt = Instrument::Nasdaq
            .signal_bar_start_utc(date(2024, 11, 3))
            .unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_dow_dst_transition_spring_2024() {
        // America/New_York DST transition: 2024-03-10 at 02:00 -> 03:00.
        // 09:45 on that date is in EDT (UTC-4) = 13:45 UTC.
        let dt = Instrument::Dow
            .signal_bar_start_utc(date(2024, 3, 10))
            .unwrap();
        assert_eq!(dt.hour(), 13);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_dow_dst_transition_autumn_2024() {
        // America/New_York DST transition: 2024-11-03 at 02:00 -> 01:00.
        // 09:45 on that date is in EST (UTC-5) = 14:45 UTC.
        let dt = Instrument::Dow
            .signal_bar_start_utc(date(2024, 11, 3))
            .unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_signal_bar_consistency_across_winter_dates() {
        // All winter dates should yield the same UTC hour/minute for a given instrument.
        let winter_dates = [date(2024, 1, 2), date(2024, 2, 15), date(2024, 12, 10)];
        for d in winter_dates {
            let dax = Instrument::Dax.signal_bar_start_utc(d).unwrap();
            assert_eq!(
                (dax.hour(), dax.minute()),
                (8, 15),
                "DAX winter failed on {d}"
            );

            let ftse = Instrument::Ftse.signal_bar_start_utc(d).unwrap();
            assert_eq!(
                (ftse.hour(), ftse.minute()),
                (8, 15),
                "FTSE winter failed on {d}"
            );

            let nq = Instrument::Nasdaq.signal_bar_start_utc(d).unwrap();
            assert_eq!(
                (nq.hour(), nq.minute()),
                (14, 45),
                "NQ winter failed on {d}"
            );
        }
    }

    #[test]
    fn test_signal_bar_consistency_across_summer_dates() {
        // All summer dates should yield the same UTC hour/minute for a given instrument.
        let summer_dates = [date(2024, 6, 3), date(2024, 7, 22), date(2024, 8, 14)];
        for d in summer_dates {
            let dax = Instrument::Dax.signal_bar_start_utc(d).unwrap();
            assert_eq!(
                (dax.hour(), dax.minute()),
                (7, 15),
                "DAX summer failed on {d}"
            );

            let ftse = Instrument::Ftse.signal_bar_start_utc(d).unwrap();
            assert_eq!(
                (ftse.hour(), ftse.minute()),
                (7, 15),
                "FTSE summer failed on {d}"
            );

            let nq = Instrument::Nasdaq.signal_bar_start_utc(d).unwrap();
            assert_eq!(
                (nq.hour(), nq.minute()),
                (13, 45),
                "NQ summer failed on {d}"
            );
        }
    }

    #[test]
    fn test_signal_bar_date_component() {
        // The returned DateTime should have the same date as the input.
        let d = date(2024, 6, 15);
        let dt = Instrument::Dax.signal_bar_start_utc(d).unwrap();
        assert_eq!(dt.date_naive(), d);
    }

    #[test]
    fn test_all_instruments() {
        assert_eq!(Instrument::ALL.len(), 4);
        assert_eq!(Instrument::ALL[0], Instrument::Dax);
        assert_eq!(Instrument::ALL[3], Instrument::Dow);
    }

    #[test]
    fn test_serde_roundtrip() {
        let json = serde_json::to_string(&Instrument::Nasdaq).unwrap();
        let parsed: Instrument = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Instrument::Nasdaq);
    }

    #[test]
    fn test_exchange_timezones() {
        assert_eq!(
            Instrument::Dax.exchange_timezone(),
            chrono_tz::Europe::Berlin
        );
        assert_eq!(
            Instrument::Ftse.exchange_timezone(),
            chrono_tz::Europe::London
        );
        assert_eq!(
            Instrument::Nasdaq.exchange_timezone(),
            chrono_tz::America::New_York
        );
        assert_eq!(
            Instrument::Dow.exchange_timezone(),
            chrono_tz::America::New_York
        );
    }

    #[test]
    fn test_market_open_local() {
        assert_eq!(
            Instrument::Dax.market_open_local(),
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        );
        assert_eq!(
            Instrument::Ftse.market_open_local(),
            NaiveTime::from_hms_opt(8, 0, 0).unwrap()
        );
        assert_eq!(
            Instrument::Nasdaq.market_open_local(),
            NaiveTime::from_hms_opt(9, 30, 0).unwrap()
        );
    }

    #[test]
    fn test_market_close_local() {
        assert_eq!(
            Instrument::Dax.market_close_local(),
            NaiveTime::from_hms_opt(17, 30, 0).unwrap()
        );
        assert_eq!(
            Instrument::Ftse.market_close_local(),
            NaiveTime::from_hms_opt(16, 30, 0).unwrap()
        );
        assert_eq!(
            Instrument::Nasdaq.market_close_local(),
            NaiveTime::from_hms_opt(16, 0, 0).unwrap()
        );
    }

    use chrono::Timelike;
}
