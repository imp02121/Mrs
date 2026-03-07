CREATE TABLE otp_requests (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL,
    otp_hash        TEXT NOT NULL,           -- Argon2 hash
    attempts        SMALLINT NOT NULL DEFAULT 0,
    max_attempts    SMALLINT NOT NULL DEFAULT 3,
    expires_at      TIMESTAMPTZ NOT NULL,
    consumed        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_otp_requests_email_active
    ON otp_requests (email, expires_at)
    WHERE consumed = FALSE;
