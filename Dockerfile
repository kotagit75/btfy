FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libgmp-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libgmp10 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/btfy /usr/local/bin/btfy

EXPOSE 8080 62697

VOLUME ["/app/node"]

CMD ["btfy", "--mining", "--beacon-cmd", "python3", "--beacon-cmd", "example/open-meteo.py"]
