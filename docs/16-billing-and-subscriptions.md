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
  [`database/migrations/0002_billing.sql`](../database/migrations/0002_billing.sql))
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
  acted on, so a retried delivery is a no-op. No RLS `select` policy at
  all; only the backend's `BYPASSRLS` system connection can touch it (see
  [10](10-database-and-security-design.md)).

## Flow

1. An organisation signs up and selects a plan (frontend calls Paystack's
   client-side inline/popup flow, or is redirected to a Paystack-hosted
   page — not yet decided which).
2. Paystack sends webhooks (`charge.success`, `subscription.create`,
   `subscription.disable`, `invoice.payment_failed`, …) to the backend's
   `POST /webhooks/paystack` (`crates/backend/src/paystack.rs`).
3. The handler verifies the `x-paystack-signature` header (HMAC-SHA512
   over the raw body — verified against raw bytes, never a re-parsed
   copy), records the event for idempotency, then applies it: currently
   `charge.success` marks the matching invoice paid, and
   `subscription.disable`/`subscription.not_renew` cancels the
   subscription. Other event types are recorded but not yet acted on —
   extend `apply_event` as billing flows need them (dunning on
   `invoice.payment_failed`, plan-change handling, etc.).
4. `organization_subscriptions.status` is what the frontend reads (via a
   backend endpoint — not built yet — scoped to the caller's own
   organisation) to decide whether to show a paywall, a "past due"
   banner, or full access.

## Platform ownership (2026-09-15)

One account — the platform operator, not a customer tenant — can see
and manage every organization: list tenants, view their users and
billing/subscription state, view access history, and deactivate a
tenant. This is `users.is_platform_owner`
([0004_platform_ownership.sql](../database/migrations/0004_platform_ownership.sql)),
checked by `crates/backend/src/routes/platform.rs`'s
`/api/v1/platform/*` endpoints — deliberately not another
organization-scoped role, since the whole point is seeing *across*
organizations, which the per-tenant `roles`/`role_assignments` RBAC
can't express.

This also resolved two of the "not decided" items below, at least for
login: a `deactivated` organization can't log in, and login is
rejected once `organization_subscriptions.status = 'trialing'` and
`current_period_end` has passed — except for the platform owner's own
organization, which is exempt by construction. Trial length is a
per-`organization_subscriptions` row (`current_period_start`/
`current_period_end`), set to 365 days for the platform owner's own
account; **48 hours is the intended default for a newly-signed-up
tenant, but nothing creates that row automatically yet** — it only
exists today because it was inserted by hand for the platform owner's
bootstrap. That's the signup-sequencing gap below, still open.

## What's not decided yet

- **Enforcement beyond login**: a request already in flight to a
  deactivated/expired organization isn't currently blocked — only the
  login endpoint checks. Whether other endpoints should re-check per
  request (and whether `past_due`, not just `expired`, should also
  block) isn't decided.
- **Plan changes and proration**: upgrade/downgrade mid-cycle isn't
  modelled yet — `organization_subscriptions` has no history of past
  plans.
- **Org + first-admin sign-up sequencing**: the backend needs to create
  the `organizations` row and the first `users` row (with its hashed
  password) together, in one transaction, before anything else can
  reference that organisation — the exact signup endpoint contract isn't
  designed yet ([10](10-database-and-security-design.md),
  [14](14-development-roadmap.md)). This is also where the 48-hour
  tenant trial default needs to be wired up: create the
  `organization_subscriptions` row (`status = 'trialing'`,
  `current_period_end = now() + 48h`) as part of the same transaction.
