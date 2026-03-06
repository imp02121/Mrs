-- backtest_runs: completed backtest executions with summary stats.
CREATE TABLE IF NOT EXISTS backtest_runs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_id     UUID NOT NULL REFERENCES strategy_configs(id),
    instrument_id SMALLINT NOT NULL REFERENCES instruments(id),
    start_date    DATE NOT NULL,
    end_date      DATE NOT NULL,
    total_trades  INTEGER NOT NULL DEFAULT 0,
    stats         JSONB NOT NULL DEFAULT '{}',
    duration_ms   INTEGER NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_backtest_runs_config_id ON backtest_runs(config_id);
CREATE INDEX IF NOT EXISTS idx_backtest_runs_instrument_id ON backtest_runs(instrument_id);
CREATE INDEX IF NOT EXISTS idx_backtest_runs_created_at ON backtest_runs(created_at DESC);
