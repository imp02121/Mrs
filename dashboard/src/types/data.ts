/** A row from the `instruments` table. Mirrors Rust `InstrumentRow`. */
export interface InstrumentRow {
  id: number;
  symbol: string;
  name: string;
  /** e.g. "09:00" */
  open_time_local: string;
  /** e.g. "17:30" */
  close_time_local: string;
  /** IANA timezone string, e.g. "Europe/Berlin" */
  timezone: string;
  /** Decimal string */
  tick_size: string;
}

/** A row from the `candles` table. Mirrors Rust `CandleRow`. */
export interface CandleRow {
  instrument_id: number;
  /** ISO 8601 UTC timestamp */
  timestamp: string;
  /** Decimal string */
  open: string;
  /** Decimal string */
  high: string;
  /** Decimal string */
  low: string;
  /** Decimal string */
  close: string;
  volume: number;
}

/** Query parameters for the candle endpoint. Mirrors Rust `CandleQuery`. */
export interface CandleQuery {
  instrument: string;
  /** YYYY-MM-DD */
  from: string;
  /** YYYY-MM-DD */
  to: string;
}

/** Request body for `POST /api/data/fetch`. Mirrors Rust `FetchRequest`. */
export interface FetchRequest {
  instrument: string;
  /** YYYY-MM-DD */
  from: string;
  /** YYYY-MM-DD */
  to: string;
}
