# Real Estate Manager

Multi-tenant land project, plot inventory, interactive plot-map, sales,
Lipa Pole Pole (instalment) financing, and payments platform. Built to
replace an existing Excel/VBA workbook (`legacy-excel/`, kept local-only —
see below) with a real multi-user system.

Repo: https://github.com/Jemwanzs/PlotsManager

## Stack

- **Frontend**: Rust, [Leptos](https://leptos.dev/) (CSR, compiled to WASM via [Trunk](https://trunkrs.dev/)). Talks only to the backend API, never to Postgres directly.
- **Backend**: Rust, [Axum](https://github.com/tokio-rs/axum) + [sqlx](https://github.com/launchbadge/sqlx). The sole authority for authentication, authorization, tenant isolation, and business rules — see [docs/12](docs/12-api-and-integration-design.md).
- **Database**: PostgreSQL on **Railway**. Row-Level Security (`database/migrations/`) is defense-in-depth behind the backend's own checks, not the primary boundary — see [docs/10](docs/10-database-and-security-design.md).
- **Billing**: [Paystack](https://paystack.com/) for the platform's own SaaS subscription billing (an organization paying for the product) — separate from in-app customer plot payments. See [docs/16](docs/16-billing-and-subscriptions.md).
- **Hosting**: **Railway** (project `c7bee255-492d-40b6-af50-30374625b279`) for the frontend, backend, and Postgres. No Vercel, no Supabase.
- **Shared `domain` crate**: plain Rust types/enums (no I/O), used by both `frontend` and `backend` so they can never drift apart.

## Layout

```
Cargo.toml              workspace root
crates/
  domain/                shared types (Organization, Plot, PlotStatus, sales/loan accounts, billing, ...)
  backend/                Axum API: auth, business logic, Postgres access, Paystack webhooks
  frontend/               Leptos WASM app — talks only to the backend (src/api/)
database/
  migrations/             schema, applied automatically by the backend on boot
  seeds/                  static reference data, applied manually
docs/                    product & technical specification (see docs/README.md)
legacy-excel/             existing Excel/VBA workbook + exports — gitignored, local reference only
```

## Frontend-first, mock-backed

The frontend is being built ahead of the backend's real endpoints against
an in-memory mock dataset (`crates/frontend/src/api/mock.rs`), behind the
same `ApiClient` interface the real HTTP client
(`crates/frontend/src/api/http.rs`) will use — see
[docs/14](docs/14-development-roadmap.md) for the reasoning and current
status. No component talks to `mock`/`http` directly, so swapping one for
the other later doesn't touch the UI.

## Getting started

Prerequisites: [Rust](https://rustup.rs/), a local Postgres (Docker is
easiest — see [`database/README.md`](database/README.md)), and
[Trunk](https://trunkrs.dev/) + the `wasm32-unknown-unknown` target for
the frontend.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk

cp .env.example .env   # fill in DATABASE_URL, JWT_SECRET, PAYSTACK_SECRET_KEY

cargo run -p backend          # runs database/migrations/ on boot, serves on :8080
cd crates/frontend && trunk serve   # frontend dev server on :8080 (Trunk's default) — runs against api::mock, no backend calls yet
```

Production: Railway hosts the backend (Postgres plugin +
`crates/backend`, reading `DATABASE_URL`/`PORT` from Railway env vars) and
the frontend (`crates/frontend`'s Trunk build). Deployment configs land
alongside that work — not committed yet, see
[docs/14](docs/14-development-roadmap.md).

## Legacy Excel/VBA source material

`legacy-excel/` holds the original workbook, exported VBA modules, and
sample extracts used to reverse-engineer business rules. It contains real
customer, payment and personal data and is **gitignored** — it stays local
and is never pushed. Treat it as read-only reference material for the
analysis captured in `docs/02-existing-vba-system-analysis.md`.

## Documentation

Full product/technical spec lives in [`docs/`](docs/README.md), including
the phased delivery roadmap in [`docs/14-development-roadmap.md`](docs/14-development-roadmap.md).
