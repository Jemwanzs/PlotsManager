# 12 — API and Integration Design

## API shape

Axum backend exposes a JSON REST API under `/api/v1` (see
[crates/api/src/routes.rs](../crates/api/src/routes.rs)). `GET /health` is
unauthenticated liveness only. Every `/api/v1` route will require an
authenticated session scoped to one `organization_id`; the auth mechanism
itself is an open question (see [10](10-database-and-security-design.md)).

Domain types returned by the API (`domain::Organization`, `domain::Plot`,
etc.) are the same types the Leptos frontend uses, via the shared `domain`
crate — no separate hand-maintained TypeScript/Rust type duplication.

## Future payment integration readiness

Payments are captured manually in v1 (see
[08](08-payments-and-receipting.md)), but the payment/verification/
allocation/audit engine is designed to be the single path for money in,
so that future integrations plug into the *same* engine rather than
maintaining a parallel balance:

- Mobile-money C2B payments
- Bank and virtual-account collections
- Card/payment-gateway collections
- Automatic transaction matching
- Webhook processing and retry management
- Unmatched-payment queues
- Automated receipting
- Daily reconciliation
- Reversal/chargeback handling

Practically: an integrated payment should land in the same `payments`
table, go through the same `Captured → Verified → Posted` lifecycle
(with `Captured` set automatically instead of by an officer), and use the
same allocation rules — the only thing that differs is *who/what* captures
it.

## Notification channels

Email, SMS, WhatsApp, and in-app notifications are referenced throughout
approvals ([09](09-approval-workflows.md)) and collections
([08](08-payments-and-receipting.md)). Treat this as a single internal
notification service with pluggable channel adapters, driven by
organisation-configured templates — not per-feature ad hoc sends.

## Document generation

PDF statements, receipts, and certificates ([08](08-payments-and-receipting.md))
should go through one templating/rendering path, not a bespoke renderer
per document type, since the audit/versioning requirements
(recipient/channel/sender/date/delivery status/version) are identical
across all of them.
