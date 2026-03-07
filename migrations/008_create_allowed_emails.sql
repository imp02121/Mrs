CREATE TABLE allowed_emails (
    id          SERIAL PRIMARY KEY,
    email       TEXT NOT NULL UNIQUE,
    role        TEXT NOT NULL DEFAULT 'viewer',  -- 'viewer' | 'admin'
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed with initial admin
INSERT INTO allowed_emails (email, role) VALUES ('admin@example.com', 'admin');
