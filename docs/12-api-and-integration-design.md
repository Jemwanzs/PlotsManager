# 12 — API and Integration Design

## Shape: Frontend → Rust API → PostgreSQL

`crates/backend` is a conventional Rust/Axum REST API and the **only**
thing that talks to Postgres. The Leptos frontend (`crates/frontend/src/api/`)
never connects to the database directly — every read and write goes
through the backend, which is the authoritative layer for authentication,
authorization, tenant isolation, and business rules (see
[10](10-database-and-security-design.md)).

`domain` types are shared between `frontend` and `backend` so request/
response payloads and the database rows they're built from can't drift
apart silently — a `Plot` struct means the same thing on both sides of
the wire.

## Frontend-first: mock now, real API later, same interface

The UI is being built ahead of the backend's real endpoints (see
[14](14-development-roadmap.md)) against an in-memory mock
(`crates/frontend/src/api/mock.rs`) that implements the exact same method
surface the real HTTP client (`crates/frontend/src/api/http.rs`) will —
both are wrapped by one `ApiClient` enum
(`crates/frontend/src/api/mod.rs`) that every component calls through.
Swapping `ApiClient::new_mock()` for `ApiClient::new_http(base_url)` at
the single call site in `app.rs` is the entire migration once the backend
routes exist — no component is rewritten.

```
UI Components
      |
Application/State Layer (Leptos signals/resources)
      |
ApiClient (api::mock today, api::http once the backend exists)
      |
Rust REST API (crates/backend)
      |
PostgreSQL (Railway)
```

## The backend's job (progressively)

Per [10](10-database-and-security-design.md) and
[04](04-user-roles-and-permissions.md), `crates/backend` owns:

- Authentication/session handling (`crates/backend/src/auth.rs` — Argon2
  password hashing and JWT session tokens, built; not yet wired to HTTP
  handlers)
- API endpoints and request validation
- Authorization, multi-tenant isolation, RBAC, branch/project scoping
- Business rules (pricing, approval gates, repayment schedules once built)
- Database access and transactions
- Approval workflows ([09](09-approval-workflows.md))
- Audit logging
- Paystack webhook verification and processing (built —
  `crates/backend/src/paystack.rs`)
- File/document operations, once an object storage backend is chosen
  ([10](10-database-and-security-design.md#file-storage))
- Reporting services ([11](11-reports-and-analytics.md))

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
allocation rules. Any such integration's webhook handling follows the
same signature-verification + idempotency pattern already established for
Paystack in `crates/backend/src/paystack.rs`.

**Do not confuse this with Paystack**, which is exclusively for the
platform's own SaaS subscription billing — see
[16](16-billing-and-subscriptions.md).

## Notification channels

Email, SMS, WhatsApp, and in-app notifications are referenced throughout
approvals ([09](09-approval-workflows.md)) and collections
([08](08-payments-and-receipting.md)). Treat this as a single internal
notification service in `crates/backend`, with pluggable channel
adapters, driven by organisation-configured templates — not per-feature
ad hoc sends. Provider not yet chosen.

## Document generation

PDF statements, receipts, and certificates ([08](08-payments-and-receipting.md))
should go through one templating/rendering path in `crates/backend`, not
a bespoke renderer per document type, since the audit/versioning
requirements (recipient/channel/sender/date/delivery status/version) are
identical across all of them. Generated files go to the same object
storage backend as uploaded documents
([10](10-database-and-security-design.md#file-storage)).

## Deployment: Railway

Railway project `c7bee255-492d-40b6-af50-30374625b279` hosts the
frontend, backend, and Postgres. Deployment configs (Dockerfiles,
per-service Railway settings) aren't committed yet — deliberately
sequenced after the frontend's UI/UX work per
[14](14-development-roadmap.md)'s priority order. `crates/backend`
already reads `PORT` from the environment (Railway's convention) so it's
ready to deploy once that work starts.
