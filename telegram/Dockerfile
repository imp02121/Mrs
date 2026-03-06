FROM rust:1.85-slim AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY engine/ engine/
COPY telegram/ telegram/

RUN cargo build --release --bin sr-telegram

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sr-telegram /usr/local/bin/sr-telegram

CMD ["sr-telegram"]
