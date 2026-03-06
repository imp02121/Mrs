//! Shared domain types: candles, instruments, signals, trades, positions, and configuration.

pub mod candle;
pub mod instrument;

pub use candle::{Candle, DateRange, DateRangeError};
pub use instrument::{Instrument, ParseInstrumentError};
