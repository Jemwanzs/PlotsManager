# Real Estate Manager

Multi-tenant land project, plot inventory, interactive plot-map, sales,
Lipa Pole Pole (instalment) financing, and payments platform. Built to
replace an existing Excel/VBA workbook (`legacy-excel/`, kept local-only —
see below) with a real multi-user system.

Repo: https://github.com/Jemwanzs/PlotsManager

## Stack

- **Backend**: Rust, [Axum](https://github.com/tokio-rs/axum), [sqlx](https://github.com/launchbadge/sqlx) (Postgres), Tokio.
- **Frontend**: Rust, [Leptos](https://leptos.dev/) (CSR, compiled to WASM via [Trunk](https://trunkrs.dev/)).
- **Shared domain crate**: plain Rust types/enums (no I/O), used by both backend and frontend so they can never drift apart.
- **Database**: PostgreSQL.

## Layout

```
Cargo.toml              workspace root
crates/
  domain/                shared types (Organization, Project, Plot, PlotStatus, sales/loan accounts, ...)
  api/                    Axum backend + sqlx migrations
  frontend/               Leptos WASM app
docs/                    product & technical specification (see docs/README.md)
legacy-excel/             existing Excel/VBA workbook + exports — gitignored, local reference only
docker-compose.yml       local Postgres for development
```

## Getting started

Prerequisites: [Rust](https://rustup.rs/), Docker (for Postgres), and
[Trunk](https://trunkrs.dev/) + the `wasm32-unknown-unknown` target for the
frontend.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk

cp .env.example .env          # then edit DATABASE_URL if needed
docker compose up -d          # starts local Postgres

cargo run -p api              # backend on :8080, runs migrations on boot
cd crates/frontend && trunk serve   # frontend dev server
```

## Legacy Excel/VBA source material

`legacy-excel/` holds the original workbook, exported VBA modules, and
sample extracts used to reverse-engineer business rules. It contains real
customer, payment and personal data and is **gitignored** — it stays local
and is never pushed. Treat it as read-only reference material for the
analysis captured in `docs/02-existing-vba-system-analysis.md`.

## Documentation

Full product/technical spec lives in [`docs/`](docs/README.md), including
the phased delivery roadmap in [`docs/14-development-roadmap.md`](docs/14-development-roadmap.md).
