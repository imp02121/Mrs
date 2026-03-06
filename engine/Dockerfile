FROM rust:1.85-slim AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY engine/ engine/
COPY telegram/ telegram/

RUN cargo build --release --bin sr-engine

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sr-engine /usr/local/bin/sr-engine

ENV HOST=0.0.0.0
ENV PORT=3001

EXPOSE 3001

CMD ["sr-engine"]
