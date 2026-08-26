# 16 — Billing and Subscriptions

This doc covers the platform charging **organisations for using Real
Estate Manager** (SaaS subscription billing via Paystack). It is a
different concern from everything in [08](08-payments-and-receipting.md),
which is a **customer** paying an organisation for a **plot** — do not
conflate the two ledgers, tables, or webhook handlers. An organisation
could have zero customer payment activity and still owe a subscription
invoice, and vice versa.

## Why Paystack, and why separate

Paystack was chosen for platform billing specifically — it isn't the
customer plot-payment channel (that's manual capture today, with M-PESA/
bank/card integrations planned later per [12](12-api-and-integration-design.md)).
Keeping them separate means:

- A Paystack outage affects only the ability to *subscribe/renew*, never
  the ability to *sell a plot or record a customer payment*.
- The webhook handler, idempotency table, and reconciliation invariant
  for subscription billing (`billing_webhook_events`,
  `organization_subscriptions`, `billing_invoices` —
  [`supabase/migrations/0002_billing.sql`](../supabase/migrations/0002_billing.sql))
  are independent of the customer-payment ledger's reconciliation
  invariant in [08](08-payments-and-receipting.md).

## Model

- **`subscription_plans`**: organisation-facing pricing tiers (code,
  name, price, currency, monthly/annual, the matching Paystack plan
  code). Seeded/managed by the platform operator, not tenants.
- **`organization_subscriptions`**: one row per organisation, tracking
  its Paystack customer/subscription codes, status (`incomplete` →
  `trialing`/`active` → `past_due`/`cancelled`/`expired`), and current
  billing period.
- **`billing_invoices`**: one row per Paystack charge, keyed by Paystack's
  transaction reference.
- **`billing_webhook_events`**: idempotency ledger — every inbound
  webhook is recorded by `(provider, paystack_event_id)` before it's
  acted on, so a retried delivery is a no-op. Not exposed via PostgREST
  at all (RLS enabled, zero policies); only the `services` crate's direct
  Postgres connection can touch it.

## Flow

1. An organisation signs up and selects a plan (frontend calls Paystack's
   client-side inline/popup flow, or is redirected to a Paystack-hosted
   page — not yet decided which).
2. Paystack sends webhooks (`charge.success`, `subscription.create`,
   `subscription.disable`, `invoice.payment_failed`, …) to
   `services`' `POST /webhooks/paystack`
   (`crates/services/src/paystack.rs`).
3. The handler verifies the `x-paystack-signature` header (HMAC-SHA512
   over the raw body — verified against raw bytes, never a re-parsed
   copy), records the event for idempotency, then applies it: currently
   `charge.success` marks the matching invoice paid, and
   `subscription.disable`/`subscription.not_renew` cancels the
   subscription. Other event types are recorded but not yet acted on —
   extend `apply_event` as billing flows need them (dunning on
   `invoice.payment_failed`, plan-change handling, etc.).
4. `organization_subscriptions.status` is what the frontend reads (via
   PostgREST, RLS-scoped to the caller's own organisation) to decide
   whether to show a paywall, a "past due" banner, or full access.

## What's not decided yet

- **Enforcement**: whether a `past_due`/`expired` subscription actually
  blocks access (and to what — read-only? fully locked?) is a product
  decision, not yet made. RLS policies would need to reference
  `organization_subscriptions.status` if so.
- **Trial policy**: length, what happens at expiry, whether a card is
  required up front.
- **Plan changes and proration**: upgrade/downgrade mid-cycle isn't
  modelled yet — `organization_subscriptions` has no history of past
  plans.
- **Where the sign-up-time organisation gets created**: a new org's first
  admin user and its `organizations` row need to exist before
  Supabase Auth's `on_auth_user_created` trigger can attach a `profiles`
  row to it ([10](10-database-and-security-design.md)) — the exact
  sequencing (org row first via a privileged call, then sign-up; or
  sign-up first with a temporary org) isn't settled.
