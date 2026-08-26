# 10 — Database and Security Design

## Platform: Supabase

Postgres, Auth, and Storage are Supabase's, not self-hosted. The Leptos
frontend talks to Supabase directly over HTTPS — PostgREST for data,
GoTrue for auth — with **no application backend in the request path** for
ordinary CRUD. See [12](12-api-and-integration-design.md) for the full
shape of that, and `crates/frontend/src/supabase/` for the client.

## Multi-tenancy: Row-Level Security, not application code

Every tenant-scoped table carries `organization_id` directly, or
transitively through a foreign key chain that terminates in one (e.g.
`plots.project_id → projects.organization_id`). Because the frontend
talks to Postgres directly through PostgREST, there is no application
layer to put a scoping check in — **Postgres Row-Level Security is the
enforcement point**, not a hardening layer added later. Every tenant
table has `alter table ... enable row level security` plus a `select`
policy keyed off `public.current_organization_id()`, a `stable security
definer` SQL function that reads the caller's org from their `profiles`
row (`auth.uid()` → `profiles.organization_id`). See
[`supabase/migrations/0001_init.sql`](../supabase/migrations/0001_init.sql).

Write policies (`insert`/`update`/`delete`) are added per table as each
write workflow is actually built, not blanket "same org can write
anything" rules — a plot price change and a payment reversal have
different permission shapes (see [04](04-user-roles-and-permissions.md),
[09](09-approval-workflows.md)), and the policy should match the real
rule, not just the tenant boundary.

## Authentication and authorization

**Authentication** is Supabase Auth (GoTrue) — email/password to start,
with OAuth/magic-link available later without a schema change. Nothing in
this codebase stores or hashes a password. A `profiles` table (id = the
`auth.users` id) extends each account with `organization_id`,
`branch_id`, `full_name` — populated automatically by an `on_auth_user_created`
trigger reading `organization_id` out of the sign-up call's user metadata
(see `crates/frontend/src/supabase/auth.rs`).

**Authorization** — the role/branch/project/field-level model in
[04](04-user-roles-and-permissions.md) — is enforced by RLS policies
querying `roles` / `role_assignments`, not by trusting the frontend. The
`services` crate connects with a direct Postgres role that bypasses RLS
entirely (for Paystack webhook writes, etc.) — that bypass is Postgres
role membership, not an API key, so it only exists where the connection
string itself grants it.

## Schema (current)

Implemented in
[`supabase/migrations/`](../supabase/migrations/):

- `0001_init.sql`: `organizations`, `branches`, `roles`, `profiles`,
  `role_assignments`, `projects`, `project_map_versions`,
  `plot_status_config`, `plots`, `customers`, `plot_sales`,
  `plot_loan_accounts`, `repayment_schedule_entries`, `payments`,
  `audit_log`, plus the RLS policies and the auth trigger above.
- `0002_billing.sql`: `subscription_plans`, `organization_subscriptions`,
  `billing_invoices`, `billing_webhook_events` — SaaS billing, see
  [16](16-billing-and-subscriptions.md).

This covers Phase 2 (foundation) and the core of Phase 5/Phase A (sales,
Plot Loan Accounts, payments) from the roadmap. Not yet modelled:
approval-workflow definitions/instances, notification templates, document
storage/versioning, commission records — added as those phases start.

## Security and audit controls

- Row-Level Security policies are the primary permission enforcement
  point (see above); field-level masking (cost price, minimum price) is
  implemented as **column-level privileges or a masked view**, since
  PostgREST has no per-field policy concept — a role either can or can't
  select a column.
- **No deletion of posted financial transactions.** Corrections are a
  reversal + replacement, both rows kept — enforced by omitting a
  `delete` RLS policy on `payments` entirely (default deny) rather than
  trusting callers not to.
- Complete before/after state captured for sensitive changes
  (`audit_log.before_state` / `after_state`, JSONB) — written by Postgres
  triggers on the audited tables, not by the frontend remembering to log
  it.
- Duplicate-prevention on plot numbers, account numbers, and payment
  references via unique constraints in the schema.
- Mandatory reasons for overrides, reversals, waivers, and backdated
  entries — enforced with `not null` reason columns plus a `check`
  constraint or trigger, since there's no API layer to validate this at.
- Locked accounting periods, where configured, block backdated posting
  via a trigger checking against an org-level settings table.

## Money and identifiers

- All monetary fields are `numeric` (Postgres) / `rust_decimal::Decimal`
  (Rust) — never floating point.
- Every plot, sale, and loan account has a system-generated UUID as its
  durable identity, independent of any external reference (title number,
  receipt number) that may not exist yet or may change.

## File storage

Uploaded project plans, KYC documents, and generated PDFs go in Supabase
**Storage** buckets, with Storage's own RLS-style policies (bucket
policies keyed the same way as the table policies above) rather than
local disk or a separately-provisioned S3 bucket. `project_map_versions.source_document_path`
and similar columns store the bucket path, not a public URL.

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
scoped RBAC via RLS, field masking, before/after audit — is a net-new
capability for the business, not a hardening of an existing one.

## Open questions for a later pass

- MFA — Supabase Auth supports it; whether/when to require it per role.
- Exactly how field-level masking is implemented (masked views vs. column
  privileges vs. a `security_invoker` view per role) — needs a decision
  once the first masked field (cost/minimum price) is actually built.
- Where the `services` crate's persistent Postgres connection role sits
  in Supabase's role model (a dedicated non-`postgres` role scoped to only
  what it needs, ideally, rather than the full superuser connection
  string).
