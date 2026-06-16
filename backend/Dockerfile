# syntax=docker/dockerfile:1.7

# Multi-stage build for the core-war-backend binary.
#
# Build context MUST be the repo root, not backend/, because backend's
# Cargo.toml has a path dep on ../engine. Invoke from the repo root:
#   docker build -f backend/Dockerfile .
# or via docker-compose.prod.yml which sets context: . for you.

FROM rust:1-bookworm AS builder
WORKDIR /app

COPY engine engine
COPY backend backend

WORKDIR /app/backend
RUN cargo build --release --bin core-war-backend


FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --system app && useradd --system --gid app --create-home --shell /bin/bash app

COPY --from=builder /app/backend/target/release/core-war-backend /usr/local/bin/core-war-backend

USER app
WORKDIR /home/app

EXPOSE 3001

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://localhost:3001/health/deep || exit 1

CMD ["core-war-backend"]
