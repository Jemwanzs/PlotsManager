# 15 — Testing and Acceptance Criteria

## Test strategy by crate

- **`domain`**: pure unit tests — serialization round-trips, any invariant
  helpers added later (e.g. status transition validity). No I/O, so no
  mocking needed.
- **`api`**: integration tests against a real Postgres (docker-compose),
  run through the actual Axum router with `tower::ServiceExt::oneshot` —
  not mocked handlers. Migrations run per test database so schema drift
  can't silently pass.
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
  combination — worth a dedicated fuzz-style test once auth exists.

## Definition of done for a feature

A feature isn't done when the happy path works — it's done when: the
relevant approval gate (if any) is enforced, the audit log records the
change, permission checks reject an unauthorised role, and (for financial
features) the reconciliation invariant above still holds.
