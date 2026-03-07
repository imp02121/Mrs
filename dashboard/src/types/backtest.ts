import type {
  Direction,
  Instrument,
  PositionStatus,
  StrategyConfig,
} from "./strategy.ts";

/** Request body for `POST /api/backtest/run`. */
export interface RunBacktestRequest {
  instrument: string;
  /** YYYY-MM-DD */
  start_date: string;
  /** YYYY-MM-DD */
  end_date: string;
  config: StrategyConfig;
}

/** Response for a completed backtest run. Mirrors Rust `BacktestRunResponse`. */
export interface BacktestRunResponse {
  run_id: string;
  result: BacktestResult;
  duration_ms: number;
}

/** Complete backtest result. Mirrors Rust `BacktestResult`. */
export interface BacktestResult {
  instrument: Instrument;
  config: StrategyConfig;
  trades: Trade[];
  equity_curve: EquityPoint[];
  daily_pnl: DailyPnl[];
  stats: BacktestStats;
}

/** Comprehensive backtest statistics. Mirrors Rust `BacktestStats`. */
export interface BacktestStats {
  total_trades: number;
  winning_trades: number;
  losing_trades: number;
  win_rate: number;
  /** Decimal string */
  total_pnl: string;
  /** Decimal string */
  avg_win: string;
  /** Decimal string */
  avg_loss: string;
  /** Decimal string */
  largest_win: string;
  /** Decimal string */
  largest_loss: string;
  /** May be `"Infinity"`, `"-Infinity"`, `"NaN"`, or a number */
  profit_factor: number | string;
  /** Decimal string */
  max_drawdown: string;
  max_drawdown_pct: number;
  sharpe_ratio: number;
  sortino_ratio: number;
  calmar_ratio: number;
  max_consecutive_wins: number;
  max_consecutive_losses: number;
  avg_trade_duration_minutes: number;
  long_trades: number;
  short_trades: number;
  /** Decimal string */
  long_pnl: string;
  /** Decimal string */
  short_pnl: string;
}

/** A completed trade. Mirrors Rust `Trade`. */
export interface Trade {
  instrument: Instrument;
  direction: Direction;
  /** Decimal string */
  entry_price: string;
  /** ISO 8601 UTC timestamp */
  entry_time: string;
  /** Decimal string */
  exit_price: string;
  /** ISO 8601 UTC timestamp */
  exit_time: string;
  /** Decimal string */
  stop_loss: string;
  exit_reason: PositionStatus;
  /** Decimal string */
  pnl_points: string;
  /** Decimal string */
  pnl_with_adds: string;
  adds: AddResult[];
  /** Decimal string */
  size: string;
}

/** PnL result for a single add-on position. Mirrors Rust `AddResult`. */
export interface AddResult {
  /** Decimal string */
  price: string;
  /** ISO 8601 UTC timestamp */
  time: string;
  /** Decimal string */
  size: string;
  /** Decimal string */
  pnl_points: string;
}

/** A single point on the equity curve. Mirrors Rust `EquityPoint`. */
export interface EquityPoint {
  /** ISO 8601 UTC timestamp */
  timestamp: string;
  /** Decimal string */
  equity: string;
}

/** Daily profit and loss. Mirrors Rust `DailyPnl`. */
export interface DailyPnl {
  /** YYYY-MM-DD */
  date: string;
  /** Decimal string */
  pnl: string;
  /** Decimal string */
  cumulative: string;
}

/** Summary of a backtest run for history listing. Mirrors Rust `BacktestRunSummary`. */
export interface BacktestRunSummary {
  id: string;
  config_id: string;
  instrument_id: number;
  /** YYYY-MM-DD */
  start_date: string;
  /** YYYY-MM-DD */
  end_date: string;
  total_trades: number;
  stats: unknown;
  duration_ms: number;
  /** ISO 8601 UTC timestamp */
  created_at: string;
}

/** A single item in a comparison result. Mirrors Rust `CompareResultItem`. */
export interface CompareResultItem {
  run_id: string;
  result: BacktestResult;
  duration_ms: number;
}

/** Request body for `POST /api/backtest/compare`. */
export interface CompareRequest {
  configs: RunBacktestRequest[];
}
