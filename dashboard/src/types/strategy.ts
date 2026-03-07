/** How the initial stop loss is determined. Mirrors Rust `StopLossMode`. */
export type StopLossMode = "SignalBarExtreme" | "FixedPoints" | "Midpoint";

/** How and when positions are exited. Mirrors Rust `ExitMode`. */
export type ExitMode = "EndOfDay" | "TrailingStop" | "FixedTakeProfit" | "CloseAtTime" | "None";

/** Trade direction. Mirrors Rust `Direction`. */
export type Direction = "Long" | "Short";

/** How a position was closed. Mirrors Rust `PositionStatus`. */
export type PositionStatus =
  | "Open"
  | "StopLoss"
  | "TakeProfit"
  | "TrailingStop"
  | "EndOfDay"
  | "TimeClose"
  | "Manual";

/** Trading instrument enum. Mirrors Rust `Instrument`. */
export type Instrument = "Dax" | "Ftse" | "Nasdaq" | "Dow";

/**
 * Complete strategy configuration.
 * Mirrors the Rust `StrategyConfig` struct exactly.
 * All Decimal fields are serialized as strings in JSON.
 */
export interface StrategyConfig {
  // -- Signal Detection --
  instrument: Instrument;
  signal_bar_index: number;
  candle_interval_minutes: number;
  /** Decimal string */
  entry_offset_points: string;
  allow_both_sides: boolean;

  // -- Stop Loss --
  sl_mode: StopLossMode;
  /** Decimal string */
  sl_fixed_points: string;
  /** Decimal string */
  sl_midpoint_offset: string;
  sl_scale_with_index: boolean;
  /** Decimal string */
  sl_scale_baseline: string;

  // -- Exit Strategy --
  exit_mode: ExitMode;
  /** NaiveTime string (HH:MM:SS) */
  exit_eod_time: string;
  /** Decimal string */
  trailing_stop_distance: string;
  /** Decimal string */
  trailing_stop_activation: string;
  /** Decimal string */
  fixed_tp_points: string;
  /** NaiveTime string (HH:MM:SS) */
  close_at_time: string;

  // -- Adding to Winners --
  add_to_winners_enabled: boolean;
  /** Decimal string */
  add_every_points: string;
  max_additions: number;
  /** Decimal string */
  add_size_multiplier: string;
  move_sl_on_add: boolean;
  /** Decimal string */
  add_sl_offset: string;

  // -- Session Timing Overrides --
  /** NaiveTime string or null */
  session_open: string | null;
  /** NaiveTime string or null */
  session_close: string | null;
  /** NaiveTime string or null */
  signal_expiry_time: string | null;

  // -- Backtest Scope --
  /** NaiveDate string (YYYY-MM-DD) */
  date_from: string;
  /** NaiveDate string (YYYY-MM-DD) */
  date_to: string;
  /** Decimal string */
  initial_capital: string;
  /** Decimal string */
  position_size: string;
  /** Decimal string */
  point_value: string;
  /** Decimal string */
  commission_per_trade: string;
  /** Decimal string */
  slippage_points: string;
  /** Array of NaiveDate strings */
  exclude_dates: string[];
}
