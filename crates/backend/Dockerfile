# Build context is the repo root (Railway service setting: root
# directory "." , Dockerfile path "crates/backend/Dockerfile") — the
# whole workspace needs to be visible so `cargo build -p backend` can
# resolve `domain` and Cargo.lock, and `sqlx::migrate!` (main.rs) can see
# database/migrations at compile time. See docs/17-deployment.md.
FROM rust:1-slim-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/domain crates/domain
COPY crates/backend crates/backend
COPY crates/frontend/Cargo.toml crates/frontend/Cargo.toml
COPY database database

# `crates/frontend` is a workspace member so Cargo needs its manifest to
# resolve the workspace graph, but building it (wasm32, trunk) has no
# place in this stage — a stub binary target satisfies manifest
# auto-discovery without pulling any of that in; nothing here depends on
# `frontend` so this stub is never actually compiled.
RUN mkdir -p crates/frontend/src && printf 'fn main() {}\n' > crates/frontend/src/main.rs

RUN cargo build --release -p backend

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend

# Railway injects PORT and DATABASE_URL; JWT_SECRET and
# PAYSTACK_SECRET_KEY must be set on the service (see .env.example /
# docs/17-deployment.md) — there's no safe default for either.
CMD ["/usr/local/bin/backend"]
