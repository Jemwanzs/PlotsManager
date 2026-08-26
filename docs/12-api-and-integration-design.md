# 12 — API and Integration Design

## Shape: Supabase as backend-as-a-service, not a custom API

There is no application server between the Leptos frontend and Postgres
for ordinary CRUD. The frontend (`crates/frontend/src/supabase/`) calls
Supabase directly:

- **PostgREST** (`{SUPABASE_URL}/rest/v1/...`) for reading/writing tables —
  what a table returns or accepts is governed entirely by the Row-Level
  Security policies in [`supabase/migrations/`](../supabase/migrations/),
  described in [10](10-database-and-security-design.md).
- **GoTrue** (`{SUPABASE_URL}/auth/v1/...`) for sign-up/sign-in/sign-out,
  wrapped in `crates/frontend/src/supabase/auth.rs`.
- **Storage** for uploaded plans, KYC documents, and generated PDFs
  (bucket policies mirror the table RLS pattern).

`domain` types are shared between `frontend` and `services`, but there is
no wire-format contract to keep in sync the way a REST API normally
would — the "API" is Postgres's own schema plus PostgREST's convention
for turning it into HTTP, and RLS is what actually decides who can do
what.

## The `services` crate: what Supabase can't do

A thin Axum service (`crates/services/`) handles the handful of things
that don't fit the "frontend talks to Postgres directly" model:

- **Paystack webhooks** (`crates/services/src/paystack.rs`) — verifying
  signatures and applying subscription/invoice state needs a server-side
  secret and a place to receive an HTTP callback, neither of which a
  static frontend has. See [16](16-billing-and-subscriptions.md).
- **PDF generation** (statements, receipts, certificates —
  [08](08-payments-and-receipting.md)) — not yet built; will live here
  once statement generation starts.
- **Repayment-schedule calculation** — the amortization math for
  interest-bearing Lipa Pole Pole sales is deliberately not implemented
  yet (see [08](08-payments-and-receipting.md)'s note on this); when it
  is, it should be computed server-side here rather than trusted from the
  client, even though `domain` types could technically be shared into the
  WASM frontend for a live preview.

It connects to the same Supabase Postgres via a direct connection string
(`DATABASE_URL`) using a role that bypasses RLS — see
[10](10-database-and-security-design.md#authentication-and-authorization).
It is **not deployed to Vercel** (Vercel doesn't run a persistent Rust
process); hosting for it is an open question — see
[14](14-development-roadmap.md).

## Future payment integration readiness

Customer plot payments are captured manually in v1 (see
[08](08-payments-and-receipting.md)), but the design should still make it
easy to plug in real collection channels later without maintaining a
parallel balance:

- Mobile-money C2B payments (M-PESA)
- Bank and virtual-account collections
- Card/payment-gateway collections
- Automatic transaction matching, webhook processing and retry
  management, unmatched-payment queues, automated receipting, daily
  reconciliation, reversal/chargeback handling

Practically: an integrated payment should land in the same `payments`
table, go through the same `Captured → Verified → Posted` lifecycle (with
`Captured` set automatically instead of by an officer), and use the same
allocation rules. Any such integration's webhook handling belongs in
`services`, following the same signature-verification + idempotency
pattern already established for Paystack in
`crates/services/src/paystack.rs`.

**Do not confuse this with Paystack**, which is exclusively for the
platform's own SaaS subscription billing — see
[16](16-billing-and-subscriptions.md).

## Notification channels

Email, SMS, WhatsApp, and in-app notifications are referenced throughout
approvals ([09](09-approval-workflows.md)) and collections
([08](08-payments-and-receipting.md)). Treat this as a single internal
notification service with pluggable channel adapters, driven by
organisation-configured templates — not per-feature ad hoc sends. Where
it lives (a Supabase Edge Function, or another `services` route) is not
yet decided; Supabase Edge Functions run on Deno/TypeScript, not Rust, so
this is a real architectural choice to make deliberately rather than
default into.

## Document generation

PDF statements, receipts, and certificates ([08](08-payments-and-receipting.md))
should go through one templating/rendering path in `services`, not a
bespoke renderer per document type, since the audit/versioning
requirements (recipient/channel/sender/date/delivery status/version) are
identical across all of them. Generated files are written to Supabase
Storage, same as uploaded documents.
