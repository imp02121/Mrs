//! Shared strategy enums and types.
//!
//! These types are used across the strategy module for direction,
//! stop loss modes, exit modes, and position status tracking.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Trade direction: long (buy) or short (sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// A long (buy) position.
    Long,
    /// A short (sell) position.
    Short,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Long => f.write_str("Long"),
            Self::Short => f.write_str("Short"),
        }
    }
}

/// How the initial stop loss is determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StopLossMode {
    /// Stop at the opposite extreme of the signal bar.
    ///
    /// For a long entry, the stop is placed at the signal bar low.
    /// For a short entry, the stop is placed at the signal bar high.
    SignalBarExtreme,

    /// A fixed-distance stop in points from the entry price.
    FixedPoints,

    /// Stop at the midpoint of the signal bar, with an optional offset.
    Midpoint,
}

impl fmt::Display for StopLossMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignalBarExtreme => f.write_str("SignalBarExtreme"),
            Self::FixedPoints => f.write_str("FixedPoints"),
            Self::Midpoint => f.write_str("Midpoint"),
        }
    }
}

/// How and when positions are exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExitMode {
    /// Close all positions at the end-of-day time.
    EndOfDay,

    /// Use a trailing stop that follows favorable price movement.
    TrailingStop,

    /// Close at a fixed take-profit distance in points.
    FixedTakeProfit,

    /// Close all positions at a specific clock time.
    CloseAtTime,

    /// No automatic exit; positions run until stopped out.
    None,
}

impl fmt::Display for ExitMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndOfDay => f.write_str("EndOfDay"),
            Self::TrailingStop => f.write_str("TrailingStop"),
            Self::FixedTakeProfit => f.write_str("FixedTakeProfit"),
            Self::CloseAtTime => f.write_str("CloseAtTime"),
            Self::None => f.write_str("None"),
        }
    }
}

/// How a position was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionStatus {
    /// Position is still open.
    Open,
    /// Closed by stop loss.
    StopLoss,
    /// Closed by take profit.
    TakeProfit,
    /// Closed by trailing stop.
    TrailingStop,
    /// Closed at end of day.
    EndOfDay,
    /// Closed at a specific time.
    TimeClose,
    /// Closed manually (not used in backtesting).
    Manual,
}

impl fmt::Display for PositionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => f.write_str("Open"),
            Self::StopLoss => f.write_str("StopLoss"),
            Self::TakeProfit => f.write_str("TakeProfit"),
            Self::TrailingStop => f.write_str("TrailingStop"),
            Self::EndOfDay => f.write_str("EndOfDay"),
            Self::TimeClose => f.write_str("TimeClose"),
            Self::Manual => f.write_str("Manual"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_display() {
        assert_eq!(format!("{}", Direction::Long), "Long");
        assert_eq!(format!("{}", Direction::Short), "Short");
    }

    #[test]
    fn test_direction_serde_roundtrip() {
        let json = serde_json::to_string(&Direction::Long).unwrap();
        let parsed: Direction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Direction::Long);

        let json = serde_json::to_string(&Direction::Short).unwrap();
        let parsed: Direction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Direction::Short);
    }

    #[test]
    fn test_stop_loss_mode_display() {
        assert_eq!(
            format!("{}", StopLossMode::SignalBarExtreme),
            "SignalBarExtreme"
        );
        assert_eq!(format!("{}", StopLossMode::FixedPoints), "FixedPoints");
        assert_eq!(format!("{}", StopLossMode::Midpoint), "Midpoint");
    }

    #[test]
    fn test_stop_loss_mode_serde_roundtrip() {
        for mode in [
            StopLossMode::SignalBarExtreme,
            StopLossMode::FixedPoints,
            StopLossMode::Midpoint,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: StopLossMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_exit_mode_display() {
        assert_eq!(format!("{}", ExitMode::EndOfDay), "EndOfDay");
        assert_eq!(format!("{}", ExitMode::TrailingStop), "TrailingStop");
        assert_eq!(format!("{}", ExitMode::FixedTakeProfit), "FixedTakeProfit");
        assert_eq!(format!("{}", ExitMode::CloseAtTime), "CloseAtTime");
        assert_eq!(format!("{}", ExitMode::None), "None");
    }

    #[test]
    fn test_exit_mode_serde_roundtrip() {
        for mode in [
            ExitMode::EndOfDay,
            ExitMode::TrailingStop,
            ExitMode::FixedTakeProfit,
            ExitMode::CloseAtTime,
            ExitMode::None,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: ExitMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_position_status_display() {
        assert_eq!(format!("{}", PositionStatus::Open), "Open");
        assert_eq!(format!("{}", PositionStatus::StopLoss), "StopLoss");
        assert_eq!(format!("{}", PositionStatus::TakeProfit), "TakeProfit");
        assert_eq!(format!("{}", PositionStatus::TrailingStop), "TrailingStop");
        assert_eq!(format!("{}", PositionStatus::EndOfDay), "EndOfDay");
        assert_eq!(format!("{}", PositionStatus::TimeClose), "TimeClose");
        assert_eq!(format!("{}", PositionStatus::Manual), "Manual");
    }

    #[test]
    fn test_position_status_serde_roundtrip() {
        for status in [
            PositionStatus::Open,
            PositionStatus::StopLoss,
            PositionStatus::TakeProfit,
            PositionStatus::TrailingStop,
            PositionStatus::EndOfDay,
            PositionStatus::TimeClose,
            PositionStatus::Manual,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: PositionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_direction_equality() {
        assert_eq!(Direction::Long, Direction::Long);
        assert_ne!(Direction::Long, Direction::Short);
    }

    #[test]
    fn test_direction_clone() {
        let d = Direction::Long;
        let d2 = d;
        assert_eq!(d, d2);
    }
}
