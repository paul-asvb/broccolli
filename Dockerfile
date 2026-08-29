FROM rust:1-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked

COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux \
        -o /usr/local/bin/yt-dlp \
    && chmod a+rx /usr/local/bin/yt-dlp

COPY --from=builder /app/target/release/broccolli /app/broccolli

ENV PORT=8080
EXPOSE 8080
CMD ["/app/broccolli"]
