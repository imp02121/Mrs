/** A row from the `live_signals` table. Mirrors Rust `SignalRow`. */
export interface SignalRow {
  /** UUID string */
  id: string;
  instrument_id: number;
  /** YYYY-MM-DD */
  signal_date: string;
  /** Decimal string */
  signal_bar_high: string;
  /** Decimal string */
  signal_bar_low: string;
  /** Decimal string */
  buy_level: string;
  /** Decimal string */
  sell_level: string;
  /** e.g. "pending", "filled", "expired" */
  status: string;
  fill_details: unknown;
  /** ISO 8601 UTC timestamp */
  created_at: string;
}
