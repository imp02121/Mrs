-- live_signals: daily trading signals generated for live monitoring.
CREATE TABLE IF NOT EXISTS live_signals (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instrument_id    SMALLINT NOT NULL REFERENCES instruments(id),
    signal_date      DATE NOT NULL,
    signal_bar_high  NUMERIC(12,2) NOT NULL,
    signal_bar_low   NUMERIC(12,2) NOT NULL,
    buy_level        NUMERIC(12,2) NOT NULL,
    sell_level       NUMERIC(12,2) NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending',
    fill_details     JSONB,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (instrument_id, signal_date)
);

CREATE INDEX IF NOT EXISTS idx_live_signals_signal_date ON live_signals(signal_date DESC);
