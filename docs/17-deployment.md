# 17 — Deployment (Railway)

Railway project `c7bee255-492d-40b6-af50-30374625b279` hosts three
services: **Postgres** (Railway's managed plugin), **backend**
(`crates/backend`, Dockerfile-based), and **frontend**
(`crates/frontend`, Dockerfile-based, static). See
[12](12-api-and-integration-design.md) for why the frontend never talks
to Postgres directly.

Both Dockerfiles build from the **repo root** as context (not their own
crate directory) — they need the whole Cargo workspace (`Cargo.lock`,
`domain`, `database/migrations`) to build. Configure each Railway
service with:

- **Root Directory**: `.` (repo root)
- **Dockerfile Path**: `crates/backend/Dockerfile` or
  `crates/frontend/Dockerfile`

## Postgres

Add Railway's Postgres plugin to the project first — the backend
service needs its `DATABASE_URL`. `crates/backend` runs
`database/migrations/` automatically on boot (`sqlx::migrate!` in
`main.rs`); there's no separate migration step to run by hand.
`database/seeds/` is dev-only — never run it against the production
database.

## Backend service (`crates/backend/Dockerfile`)

Required environment variables:

| Variable | Value |
|---|---|
| `DATABASE_URL` | Railway auto-injects this if the backend service and Postgres plugin share an environment (Railway's "reference variable" feature) — otherwise copy it from the Postgres plugin's Connect tab. |
| `JWT_SECRET` | A real random value — `openssl rand -hex 32`. Never reuse the `.env.example` placeholder. Rotating it invalidates every issued session token. |
| `PAYSTACK_SECRET_KEY` | From the Paystack dashboard (live or test key depending on environment). |
| `PORT` | Railway injects this automatically — `crates/backend/src/main.rs` already reads it. Don't set `BIND_ADDR` in production; that's the local-dev-only fallback. |
| `RUST_LOG` | Optional, defaults to `backend=debug,tower_http=debug` — turn down to `backend=info,tower_http=info` once things are stable. |

No build arguments needed — nothing about the backend image is
baked-in at build time.

## Frontend service (`crates/frontend/Dockerfile`)

This is a **static** service: the Dockerfile's final stage is nginx
serving Trunk's build output, not a running Rust process. One build
argument:

| Build arg | Value |
|---|---|
| `API_BASE_URL` | The backend service's public Railway URL (e.g. `https://<backend-service>.up.railway.app`). Baked into the compiled wasm at build time via `option_env!` (`crates/frontend/src/app.rs`) — there's no runtime config for this, so **changing it requires a rebuild**, not just a redeploy. |

Railway lets you set build arguments per-service in the service's
Settings → Build tab. `PORT` is a runtime environment variable, not a
build arg — Railway injects it and the container's
`docker-entrypoint.sh` renders it into the nginx config at startup
(deliberately not nginx's own template auto-substitution, which has no
variable allowlist and would also mangle `$uri` in the config — see the
comment in `docker-entrypoint.sh`).

If `API_BASE_URL` is left unset at build time, the app falls back to
`crates/frontend/src/api/mock.rs` (in-memory demo data, no backend
calls) — safe, but not what you want for a real deployment; double
check the build arg is actually set before trusting a "successful"
frontend deploy.

## Verifying a deploy

- Backend: `GET /health` returns `{"status":"ok"}`.
- Frontend: `GET /health` returns `ok` (added in
  `nginx.conf.template` specifically so Railway's health check has
  something cheap to poll that isn't `index.html`).
- Full loop: open the frontend URL, sign in with a real seeded user
  (not the dev demo seed — that's local-only), confirm the dashboard
  loads real numbers. If it shows mock data instead, `API_BASE_URL`
  wasn't set at build time.

## Local build/run (validating the images before pushing)

```bash
# from the repo root
docker build -f crates/backend/Dockerfile -t rem-backend .
docker build -f crates/frontend/Dockerfile --build-arg API_BASE_URL=http://localhost:8095 -t rem-frontend .

docker run --rm -p 8095:8080 -e PORT=8080 \
  -e DATABASE_URL=... -e JWT_SECRET=dev -e PAYSTACK_SECRET_KEY=sk_test_x \
  rem-backend

docker run --rm -p 8090:8080 -e PORT=8080 rem-frontend
```

## Not yet done

- No CI pipeline building/pushing these images on push to `main` —
  deploys today are manual (`railway up` or connecting the GitHub repo
  to each service for auto-deploy on push, via the Railway dashboard).
- No staging environment — one Railway environment (production) only.
- The least-privilege RLS-subject Postgres role
  ([10](10-database-and-security-design.md)) isn't provisioned; the
  backend connects as the same user that owns the schema. RLS policies
  are real and applied, but aren't yet the primary enforcement boundary
  they're designed to be — see the note in
  [14](14-development-roadmap.md).
