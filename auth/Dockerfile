FROM rust:1.88-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace Cargo files
COPY Cargo.toml Cargo.lock ./

# Create dummy source files for all workspace members so cargo can resolve deps
RUN mkdir -p engine/src telegram/src auth/src \
    && echo 'fn main() {}' > engine/src/main.rs \
    && echo 'fn main() {}' > telegram/src/main.rs \
    && echo 'fn main() {}' > auth/src/main.rs \
    && echo '' > engine/src/lib.rs \
    && echo '' > telegram/src/lib.rs \
    && echo '' > auth/src/lib.rs
COPY engine/Cargo.toml engine/Cargo.toml
COPY telegram/Cargo.toml telegram/Cargo.toml
COPY auth/Cargo.toml auth/Cargo.toml

# Build dependencies only (cached layer)
RUN cargo build --release --bin sr-auth 2>/dev/null || true

# Copy real source and build
COPY engine/ engine/
COPY telegram/ telegram/
COPY auth/ auth/
RUN touch auth/src/main.rs && cargo build --release --bin sr-auth

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sr-auth /usr/local/bin/sr-auth

ENV HOST=0.0.0.0
ENV PORT=3002

EXPOSE 3002

CMD ["sr-auth"]
