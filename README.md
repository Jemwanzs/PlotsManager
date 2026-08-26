# Real Estate Manager

Multi-tenant land project, plot inventory, interactive plot-map, sales,
Lipa Pole Pole (instalment) financing, and payments platform. Built to
replace an existing Excel/VBA workbook (`legacy-excel/`, kept local-only —
see below) with a real multi-user system.

Repo: https://github.com/Jemwanzs/PlotsManager

## Stack

- **Frontend**: Rust, [Leptos](https://leptos.dev/) (CSR, compiled to WASM via [Trunk](https://trunkrs.dev/)), deployed to **Vercel** as a static site.
- **Backend-as-a-service**: [Supabase](https://supabase.com/) — Postgres, Auth, and Storage. The frontend talks to Supabase directly (PostgREST + GoTrue) over HTTPS; multi-tenancy and permissions are enforced by Postgres **Row-Level Security**, not application code. See [docs/12](docs/12-api-and-integration-design.md).
- **`services` crate**: a thin Rust/Axum service for the handful of things Supabase can't do directly — verifying and applying **Paystack** webhooks today, PDF generation and repayment-schedule calculation as those land. Not deployed to Vercel (needs a persistent Rust host — Fly.io/Shuttle/Railway; not yet provisioned).
- **Billing**: [Paystack](https://paystack.com/) for the platform's own SaaS subscription billing (an organization paying for the product) — separate from in-app customer plot payments. See [docs/16](docs/16-billing-and-subscriptions.md).
- **Shared `domain` crate**: plain Rust types/enums (no I/O), used by both `frontend` and `services` so they can never drift apart.

## Layout

```
Cargo.toml              workspace root
crates/
  domain/                shared types (Organization, Plot, PlotStatus, sales/loan accounts, billing, ...)
  services/               thin Axum service: Paystack webhooks today, PDF/schedule generation later
  frontend/               Leptos WASM app — talks to Supabase directly (src/supabase/)
supabase/
  migrations/             Postgres schema + Row-Level Security policies (Supabase CLI)
docs/                    product & technical specification (see docs/README.md)
legacy-excel/             existing Excel/VBA workbook + exports — gitignored, local reference only
vercel.json              static deploy config for crates/frontend
scripts/vercel-build.sh   installs Rust + Trunk on Vercel's build image, then builds the frontend
```

## Getting started

Prerequisites: [Rust](https://rustup.rs/), the
[Supabase CLI](https://supabase.com/docs/guides/cli) (Docker required for
local Supabase), and [Trunk](https://trunkrs.dev/) + the
`wasm32-unknown-unknown` target for the frontend.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk

supabase start                # local Supabase stack (Postgres/Auth/Storage/PostgREST)
supabase db push               # applies supabase/migrations/

cp .env.example .env
# fill in DATABASE_URL (from `supabase status`), PAYSTACK_SECRET_KEY,
# SUPABASE_URL / SUPABASE_ANON_KEY (also from `supabase status`)

cargo run -p services          # Paystack-webhook service, :8080
cd crates/frontend && trunk serve   # frontend dev server, reads SUPABASE_* from your shell env
```

Production: point a Supabase **cloud** project's connection details at the
same env vars, run `supabase db push --linked` (or push through CI),
deploy `crates/frontend` to **Vercel** (`vercel.json` + `scripts/vercel-build.sh`
handle the Rust/Trunk build), and deploy `crates/services` to a
persistent Rust host — not decided yet.

## Legacy Excel/VBA source material

`legacy-excel/` holds the original workbook, exported VBA modules, and
sample extracts used to reverse-engineer business rules. It contains real
customer, payment and personal data and is **gitignored** — it stays local
and is never pushed. Treat it as read-only reference material for the
analysis captured in `docs/02-existing-vba-system-analysis.md`.

## Documentation

Full product/technical spec lives in [`docs/`](docs/README.md), including
the phased delivery roadmap in [`docs/14-development-roadmap.md`](docs/14-development-roadmap.md).
