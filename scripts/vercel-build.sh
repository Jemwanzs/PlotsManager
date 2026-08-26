#!/usr/bin/env bash
# Vercel's build image has no Rust toolchain by default, so the build
# command installs one, then builds the Leptos frontend with Trunk.
# SUPABASE_URL / SUPABASE_ANON_KEY must be set as Vercel project
# environment variables — Trunk bakes them into the WASM bundle at build
# time via `option_env!` (crates/frontend/src/supabase/config.rs), and
# Vercel exposes project env vars to the build step automatically.
set -euo pipefail

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown

curl -L "https://github.com/trunk-rs/trunk/releases/latest/download/trunk-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xzf - -C "$HOME/.cargo/bin"

cd crates/frontend
trunk build --release
