-- instruments: supported trading instruments with session metadata.
CREATE TABLE IF NOT EXISTS instruments (
    id              SMALLINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    symbol          TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    open_time_local TEXT NOT NULL,
    close_time_local TEXT NOT NULL,
    timezone        TEXT NOT NULL,
    tick_size       NUMERIC(10,2) NOT NULL
);

-- Seed the 4 supported instruments.
INSERT INTO instruments (symbol, name, open_time_local, close_time_local, timezone, tick_size)
VALUES
    ('DAX',  'DAX 40',            '09:00', '17:30', 'Europe/Berlin',    0.50),
    ('FTSE', 'FTSE 100',          '08:00', '16:30', 'Europe/London',    0.50),
    ('IXIC', 'Nasdaq Composite',  '09:30', '16:00', 'America/New_York', 0.25),
    ('DJI',  'Dow Jones',         '09:30', '16:00', 'America/New_York', 1.00)
ON CONFLICT (symbol) DO NOTHING;
