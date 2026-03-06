-- strategy_configs: saved strategy parameter sets.
CREATE TABLE IF NOT EXISTS strategy_configs (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    params     JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
