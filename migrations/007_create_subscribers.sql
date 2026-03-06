-- subscribers: Telegram bot subscribers for signal notifications.
CREATE TABLE IF NOT EXISTS subscribers (
    id                      SERIAL PRIMARY KEY,
    chat_id                 BIGINT NOT NULL UNIQUE,
    username                TEXT,
    subscribed_instruments  TEXT[] NOT NULL DEFAULT '{}',
    active                  BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_subscribers_active ON subscribers(active) WHERE active = true;
