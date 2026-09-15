# Build context is the repo root (Railway service setting: root
# directory ".", Dockerfile path "crates/frontend/Dockerfile") — see
# docs/17-deployment.md. Two stages: compile the Leptos app to wasm with
# Trunk, then serve the static output with nginx (this is a CSR app —
# there's no server-side Rust process to run here, just static files).
FROM rust:1-slim-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown
RUN curl -sL https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-x86_64-unknown-linux-gnu.tar.gz \
    | tar xz -C /usr/local/bin
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/domain crates/domain
COPY crates/frontend crates/frontend
COPY crates/backend/Cargo.toml crates/backend/Cargo.toml

# `crates/backend` is a workspace member so Cargo needs its manifest to
# resolve the workspace graph; its source (tokio/axum/sqlx, none of it
# wasm32-safe) has no place in this stage and is never compiled here
# since nothing in the frontend's build depends on it.
RUN mkdir -p crates/backend/src && printf 'fn main() {}\n' > crates/backend/src/main.rs

# Baked into the wasm binary at compile time (see
# crates/frontend/src/app.rs's `option_env!("API_BASE_URL")`) — must be
# the backend service's public URL, passed as a build arg since this is
# a static build, not a runtime config.
ARG API_BASE_URL
ENV API_BASE_URL=${API_BASE_URL}

WORKDIR /app/crates/frontend
RUN trunk build --release

FROM nginx:1.27-alpine
RUN apk add --no-cache curl
COPY --from=builder /app/crates/frontend/dist /usr/share/nginx/html
COPY crates/frontend/nginx.conf.template /etc/nginx/templates/default.conf.template
COPY crates/frontend/docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh

ENV PORT=8080
EXPOSE 8080
ENTRYPOINT ["/docker-entrypoint.sh"]
