-- trades: individual trades belonging to a backtest run.
CREATE TABLE IF NOT EXISTS trades (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    backtest_run_id UUID NOT NULL REFERENCES backtest_runs(id) ON DELETE CASCADE,
    instrument_id   SMALLINT NOT NULL REFERENCES instruments(id),
    direction       TEXT NOT NULL CHECK (direction IN ('Long', 'Short')),
    entry_price     NUMERIC(12,2) NOT NULL,
    entry_time      TIMESTAMPTZ NOT NULL,
    exit_price      NUMERIC(12,2) NOT NULL,
    exit_time       TIMESTAMPTZ NOT NULL,
    stop_loss       NUMERIC(12,2) NOT NULL,
    exit_reason     TEXT NOT NULL,
    pnl_points      NUMERIC(12,2) NOT NULL,
    pnl_with_adds   NUMERIC(12,2) NOT NULL,
    adds            JSONB NOT NULL DEFAULT '[]',
    trade_date      DATE NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_trades_backtest_run_id ON trades(backtest_run_id);
CREATE INDEX IF NOT EXISTS idx_trades_instrument_id ON trades(instrument_id);
CREATE INDEX IF NOT EXISTS idx_trades_trade_date ON trades(trade_date);
