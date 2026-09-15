# Database

Platform-neutral PostgreSQL — runs on Railway Postgres in production, any
Postgres 14+ locally. No Supabase/managed-BaaS dependency: authentication,
authorization, and tenant isolation are enforced by `crates/backend`, not
by the database layer talking directly to a frontend. See
[`docs/10-database-and-security-design.md`](../docs/10-database-and-security-design.md)
for the full design rationale, including how Row-Level Security is used
here as defense-in-depth rather than the primary boundary.

## Layout

```
database/
├── migrations/   schema, applied automatically by the backend on boot
│                 (sqlx::migrate!, see crates/backend/src/main.rs)
├── seeds/        static reference data (e.g. subscription plans) —
│                 run manually, never applied automatically
└── README.md     this file
```

## Running migrations

Migrations run automatically when `crates/backend` starts
(`sqlx::migrate!("../../database/migrations")`), against whatever
`DATABASE_URL` points at. For a one-off run without starting the full
service (e.g. in CI, or before first deploy):

```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
sqlx migrate run --source database/migrations --database-url "$DATABASE_URL"
```

To add a migration, create a new numbered file
(`database/migrations/000N_description.sql`) — numbers must sort
correctly and never be reused or edited once applied anywhere beyond your
own machine, since `sqlx::migrate!` tracks applied migrations by
filename+checksum in a `_sqlx_migrations` table.

## Seeding

```bash
psql "$DATABASE_URL" -f database/seeds/0001_subscription_plans.sql
```

## Local development

Any Postgres 14+ works — either a Docker container or a native install.

```bash
docker run -d --name rem-postgres -p 5432:5432 \
    -e POSTGRES_PASSWORD=postgres \
    postgres:16
createdb -h 127.0.0.1 -U postgres real_estate_manager   # dedicated db, not the default "postgres" one
```

Then set `DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/real_estate_manager`
in `.env` (see `.env.example`). Two things worth knowing before you hit
either:

- **Use a dedicated database name**, not the connection's default
  `postgres` database — that one is easy to end up sharing with whatever
  else runs locally on the same Postgres instance, and migrations don't
  namespace themselves.
- **Prefer `127.0.0.1` over `localhost`.** If a native Postgres service
  is already running on the machine (common on Windows — a PostgreSQL
  installer registers itself as a service bound to `0.0.0.0:5432`),
  `localhost` can resolve to `::1` and land on *that* service instead of
  your Docker container, even though the connection succeeds and looks
  fine — you just end up migrating and querying the wrong server
  entirely, silently. `127.0.0.1` forces IPv4 and avoids the ambiguity.
  (Symptom if this happens to you: `sqlx::migrate!` logs no error and the
  app starts, but the tables never appear where you're looking for them.)
