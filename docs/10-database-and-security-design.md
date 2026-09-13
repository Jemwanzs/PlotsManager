# 10 — Database and Security Design

## Platform: Railway Postgres + a Rust backend

Postgres is Railway's managed Postgres, not a BaaS. There is no Auth-as-
a-service and no auto-generated database API — `crates/backend` is the
**only** thing that talks to Postgres, and the **primary** enforcement
point for authentication, authorization, and tenant isolation. The
frontend never connects to the database directly; every request goes
through the backend's own HTTP API. See
[12](12-api-and-integration-design.md) for the full shape of that.

(This project briefly targeted Supabase + Vercel — see git history around
2026-09 if that context is ever needed. It's not the target architecture;
don't resurrect it.)

## Multi-tenancy: backend-enforced, RLS as defense in depth

Every tenant-scoped table carries `organization_id` directly, or
transitively through a foreign key chain that terminates in one (e.g.
`plots.project_id → projects.organization_id`). The backend is
responsible for scoping every query to the authenticated caller's
organisation — that is the real boundary. Row-Level Security policies in
[`database/migrations/0001_init.sql`](../database/migrations/0001_init.sql)
exist as a **second, independent layer**: the backend opens a
transaction and runs `select set_config('app.current_organization_id',
$1, true)` with the caller's org id (from their verified session token)
before any tenant-scoped query, and every tenant table's RLS policy
checks `organization_id = public.current_org_id()`, a helper that reads
that session-local setting. A bug that forgets a `WHERE` clause in
application code still can't leak across tenants; a bug that forgets to
set the session variable gets zero rows back, not someone else's data
(`current_setting(..., true)` is null when unset, and null never equals
an `organization_id`).

This requires two distinct Postgres roles in principle: an ordinary,
RLS-subject role for request-scoped queries (sets the session variable,
gets policy-filtered results) and a role with `BYPASSRLS` for
system/admin operations that aren't tied to one tenant's session
(migrations, Paystack webhook processing). **Only the `BYPASSRLS` side of
this is provisioned today** — the backend currently uses one Postgres
connection (from `DATABASE_URL`) for everything, including the
not-yet-built request-scoped queries. Provisioning the least-privilege
`app_user` role and wiring the session-variable-setting middleware is an
open task for the "Rust APIs & Authentication" roadmap phase (see
[14](14-development-roadmap.md)) — the schema and RLS policies are ready
for it now so that phase is wiring, not schema design.

Write policies (`insert`/`update`/`delete`) are added per table as each
write workflow is actually built, not blanket "same org can write
anything" rules — a plot price change and a payment reversal have
different permission shapes (see [04](04-user-roles-and-permissions.md),
[09](09-approval-workflows.md)), and the policy should match the real
rule, not just the tenant boundary.

## Authentication and authorization

**Authentication** is owned entirely by the backend —
`crates/backend/src/auth.rs` has working, tested Argon2id password
hashing and JWT session-token issuance/verification. Nothing outside this
codebase stores or hashes a password, and there's no third-party auth
dependency. `users.password_hash` holds the Argon2id hash; a JWT carries
`sub` (user id) and `organization_id` so the backend can set the RLS
session variable without an extra lookup query per request. Signup/login
HTTP handlers aren't built yet (see [14](14-development-roadmap.md)) —
the primitives are complete, wiring them to routes is the remaining work.

**Authorization** — the role/branch/project/field-level model in
[04](04-user-roles-and-permissions.md) — is enforced by the backend
checking `roles`/`role_assignments` before performing an action, not by
trusting the frontend or relying on RLS alone (RLS is tenant-boundary
defense in depth, not a substitute for permission checks like "can this
role approve a discount over X").

## Schema (current)

Implemented in [`database/migrations/`](../database/migrations/), applied
automatically by the backend on boot (`sqlx::migrate!`, see
`crates/backend/src/main.rs`) — no external migration CLI or dashboard
required:

- `0001_init.sql`: `organizations`, `branches`, `roles`, `users`,
  `role_assignments`, `projects`, `project_map_versions`,
  `plot_status_config`, `plots`, `customers`, `plot_sales`,
  `plot_loan_accounts`, `repayment_schedule_entries`, `payments`,
  `audit_log`, plus the RLS policies and `current_org_id()` helper above.
- `0002_billing.sql`: `subscription_plans`, `organization_subscriptions`,
  `billing_invoices`, `billing_webhook_events` — SaaS billing, see
  [16](16-billing-and-subscriptions.md).

This covers Phase 2 (foundation) and the core of Phase 5/Phase A (sales,
Plot Loan Accounts, payments) from the roadmap. Not yet modelled:
approval-workflow definitions/instances, notification templates, document
storage/versioning, commission records — added as those phases start. See
[`database/README.md`](../database/README.md) for how to run migrations
and seeds directly.

## Security and audit controls

- Backend-enforced permission checks are the primary control (see above);
  field-level masking (cost price, minimum price) is applied by the
  backend at the API response layer — it simply omits the field for a
  role that shouldn't see it, rather than relying on the database to
  mask it.
- **No deletion of posted financial transactions.** Corrections are a
  reversal + replacement, both rows kept — the backend never exposes a
  delete operation for `payments`, and there's no RLS `delete` policy on
  it either (defense in depth: even a compromised backend connection
  using the RLS-subject role can't delete one).
- Complete before/after state captured for sensitive changes
  (`audit_log.before_state` / `after_state`, JSONB) — written by the
  backend as part of the same transaction as the change, not a
  best-effort afterthought.
- Duplicate-prevention on plot numbers, account numbers, and payment
  references via unique constraints in the schema, not just application
  checks.
- Mandatory reasons for overrides, reversals, waivers, and backdated
  entries — validated by the backend before the query is even issued.
- Locked accounting periods, where configured, block backdated posting —
  a backend-side check against an org-level settings table.

## Money and identifiers

- All monetary fields are `numeric` (Postgres) / `rust_decimal::Decimal`
  (Rust) — never floating point.
- Every plot, sale, and loan account has a system-generated UUID as its
  durable identity, independent of any external reference (title number,
  receipt number) that may not exist yet or may change.

## File storage

Uploaded project plans, KYC documents, and generated PDFs need an object
storage backend — not yet decided. Options: a Railway persistent volume
(simplest, tied to one service instance), or an S3-compatible bucket
(Railway doesn't offer one natively; would be an external provider).
Whichever is chosen, the backend proxies all access (upload, download,
delete) rather than handing the frontend a direct storage URL, so the
same authorization checks apply to files as to database rows.
`project_map_versions.source_document_path` and similar columns store an
internal storage key, not a public URL.

## Legacy reality (see [02](02-existing-vba-system-analysis.md))

The system being replaced authenticates against **up to five hardcoded
username/password pairs stored in plaintext in a worksheet cell** — no
hashing, no lockout, no password policy. Authorization is a single
hardcoded check (`username = "Admin"`) gating one screen; every other
screen and every field (including cost price and minimum price) is open
to anyone logged in. The only audit trail is a navigation log (who opened
which screen, when) — there is no field-level before/after record of data
changes, so "who changed this price and what was it before" is
unanswerable today. Every control in this document — real authentication,
scoped RBAC, field masking, before/after audit — is a net-new capability
for the business, not a hardening of an existing one.

## Open questions for a later pass

- Provisioning the least-privilege `app_user` Postgres role and the
  request-scoped session-variable middleware (see above) — the concrete
  next step once signup/login handlers are built.
- MFA — whether/when to require it per role.
- Object storage backend for uploaded/generated files (see above).
- Railway secret management specifics for `DATABASE_URL`, `JWT_SECRET`,
  `PAYSTACK_SECRET_KEY` across dev/staging/production environments within
  the Railway project.
