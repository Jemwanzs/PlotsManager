# 15 — Testing and Acceptance Criteria

## Test strategy by crate

- **`domain`**: pure unit tests — serialization round-trips, any invariant
  helpers added later (e.g. status transition validity). No I/O, so no
  mocking needed.
- **`services`**: integration tests against a real Postgres (a local
  `supabase start` instance, or plain Postgres with `supabase/migrations/`
  applied), run through the actual Axum router with
  `tower::ServiceExt::oneshot` — not mocked handlers. For the Paystack
  webhook route specifically: test signature verification with both a
  valid and a tampered body, and test that replaying the same event id is
  a no-op against `billing_webhook_events`'s unique constraint.
- **Row-Level Security policies** (`supabase/migrations/`): a distinct,
  important test category now that RLS is the primary enforcement point
  instead of application code — see
  [10](10-database-and-security-design.md). For each tenant table:
  connect as two different organisations' users (via short-lived test
  JWTs or `set local role`/`set request.jwt.claims`) and assert org A
  cannot read or write org B's rows, in both directions. This is the
  direct replacement for what would otherwise be application-layer
  authorization tests.
- **`frontend`**: component-level tests where Leptos's testing story
  supports it; otherwise rely on manual verification in the browser via
  `trunk serve` for interactive map/polygon-editor behaviour, which is
  inherently hard to unit test meaningfully.

## Acceptance criteria drawn from the business rules

These map directly to the "key business rules" scattered through
[07](07-sales-and-booking-workflows.md)–[10](10-database-and-security-design.md)
— each one should become an actual integration test once the relevant
feature is built, not stay as prose:

- Every sale links to exactly one customer, project, and plot.
- A plot cannot have more than one active sale (absent explicit joint
  ownership).
- A payment only affects the official balance after reaching the required
  verification/approval status — never before.
- Posted payments cannot be edited or deleted; a correction always
  produces a reversal entry alongside the original, both retained.
- Interest, charges, and penalties are only ever generated from an
  approved configuration.
- Fully-paid status is system-calculated, not manually set, and honours
  any configured settlement-confirmation approval step.
- Title transfer cannot begin until all configured financial, legal, and
  approval conditions are met.
- Dashboard totals, statements, plot accounts, and reports reconcile to
  the same underlying transaction ledger (a strong integration-test
  candidate: seed transactions, assert the dashboard aggregate and the
  statement total agree exactly).
- Plot numbers, title numbers, loan account numbers, and payment
  references reject duplicates at the database constraint level, not just
  in application code.
- A published map version is immutable; editing always produces a new
  draft version, never mutates the approved one in place.
- Cross-tenant data isolation: a query authenticated as one organisation
  can never return another organisation's rows, under any filter
  combination — this is now an RLS policy test (see above), not an
  application-code test, since PostgREST queries hit Postgres directly.

## Definition of done for a feature

A feature isn't done when the happy path works — it's done when: the
relevant approval gate (if any) is enforced, the audit log records the
change, permission checks reject an unauthorised role, and (for financial
features) the reconciliation invariant above still holds.
