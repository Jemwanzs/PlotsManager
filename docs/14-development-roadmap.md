# 14 — Development Roadmap

Two roadmaps from the original scoping conversation are merged here: the
platform-wide phases (1–7) and the payments-specific delivery sequence
(A–D), which nests inside phases 5–7.

## Current status (2026-09-13)

**Architecture (final): Frontend → Rust API → PostgreSQL, all on
Railway** (project `c7bee255-492d-40b6-af50-30374625b279`). This project
briefly targeted Supabase + Vercel (2026-08-26 to 2026-09-13); that's been
fully refactored away — the Leptos frontend never talks to the database,
`crates/backend` (Axum) owns authentication/authorization/tenant
isolation, and Postgres lives on Railway with Row-Level Security as
defense-in-depth, not the enforcement point. See
[10](10-database-and-security-design.md) and
[12](12-api-and-integration-design.md).

The Cargo workspace is `domain` (shared types, including billing),
`backend` (Axum — health check, Paystack webhook receiver, and working
auth primitives in `auth.rs` not yet wired to routes), and `frontend`
(Leptos CSR shell with routing, a responsive app shell, and real screens
— login, dashboard, projects, project/plot detail — built against an
in-memory mock dataset behind the same interface the real API will use,
see `crates/frontend/src/api/`). `database/migrations/` holds the schema
and RLS policies, applied automatically by the backend on boot.

**Priority order for what's next** (per the 2026-09-13 architecture
decision): Frontend/UI → Complete User Journeys → Mobile/Responsive
Polish → Railway Frontend Deployment → PostgreSQL/Migrations → Rust APIs
& Authentication → Frontend/API Integration → External Integrations →
Testing/Security → Production Hardening. Backend/database work continues
in parallel where useful but doesn't block frontend progress.

## Phase 1 — Discovery and Legacy Analysis
Analyse the Excel/VBA system, extract business rules, document current
workflows, identify migration requirements, produce full specs.
**Status: complete.** 58 VBA modules exported and analysed — see
[02](02-existing-vba-system-analysis.md) for the full data model,
numbering rules, workflow behavior, security posture, and a gap-analysis
table mapping legacy behavior to every affected spec doc (03, 04, 05, 08,
09, 10, 11, 13). One open product decision surfaced: whether to carry
forward the legacy customer feedback/ratings module, currently unspecified
anywhere else in `docs/`.

## Phase 2 — Platform Foundation
Multi-tenant architecture; organisation settings; users, roles,
permissions; projects and plot register; documents and audit logs;
configurable numbering.
**Status: infrastructure decided (Railway: frontend + Rust API +
Postgres), schema + RLS policies + domain types scaffolded, backend auth
primitives (Argon2 + JWT, tested) built but not wired to routes, frontend
has a real login screen and app shell against mock auth. Not yet built:
actual signup/login HTTP endpoints, org creation flow, numbering config,
document storage wiring, the least-privilege RLS-subject Postgres role
(see [10](10-database-and-security-design.md)), or Railway deployment
configs for `backend`/`frontend`. The plot register itself has real
create flows against mock data now — new project, new plot (per-project
unique plot numbers, fixing the legacy global-uniqueness bug from
[02](02-existing-vba-system-analysis.md) §3), new customer — each with
the validation the schema itself enforces (duplicate codes/IDs rejected)
replicated in the mock so the UI behaves the same way the real backend
will once it exists.**

## Phase 3 — Interactive Maps
Upload project plans; manual polygon drawing; plot-to-map linking;
colour-coded statuses; search/filter/pan/zoom; map versioning and
approvals.
**Status: not started** (schema for `project_map_versions` exists).

## Phase 4 — AI-Assisted Plan Conversion
Image enhancement; OCR; boundary detection; plot-number recognition;
confidence scores; exception handling and human correction.
**Status: not started — deliberately sequenced after Phase 3.**

## Phase 5 — Sales and Customer Management
Leads/prospects; plot selection; holds/reservations/bookings; quotations/
offer letters; sales agreements; customer 360°; agent assignment/
commissions.
**Status: frontend groundwork started against mock data.** Customers
list + detail (purchase history) and a "Reserve this plot" flow exist —
picking a customer and payment mode on an uncommitted plot creates a
mock `PlotSale`, moves the plot to Reserved/Booked, and the plot grid
updates live. Not built: leads/prospects, holds vs. reservations as
distinct stages, quotations/offer letters, agent commissions, a real
customer 360° view (today's customer detail is purchase history only).

## Phase 6 — Payments and Transfers
Nests the payments delivery sequence:

- **Phase A** — cash and Lipa Pole Pole sale modes, Plot Loan Accounts,
  repayment schedules, manual payment capture/allocation, approvals/
  reversals, receipts/statements.
- **Phase B** — arrears ageing, notifications/work queues, customer 360°
  financial view, dashboards, report library.
- **Phase C** — restructures/waivers/holidays, cancellations/repossessions/
  reallocations, agent commissions, title-transfer readiness workflows.
- **Phase D** — mobile-money/banking integrations, automated matching/
  receipting/reconciliation, customer self-service portal.

**Status: Phase A frontend groundwork started against mock data.** A
Plot Loan Account is created automatically when a Lipa Pole Pole sale is
reserved (fixed 10% deposit / 12 monthly instalments — no tenor/deposit
picker yet), with a detail screen (balance, instalment, deposit) and a
"Record a payment" form that updates the running balance and status
(Awaiting Deposit → Active (Partially Paid) → Fully Paid) live. Not
built: a real generated repayment schedule
(`repayment_schedule_entries` — see docs/08's note that the interest/
amortization engine is deliberately deferred), the Captured → Verified →
Posted approval lifecycle (payments post immediately, no approval gate
yet — that needs docs/09's engine and real authenticated roles first),
receipts/statements, arrears ageing, and everything in Phases B–D.

## Phase 7 — Analytics and Integrations
Project-performance dashboards; plot-availability analytics; sales
conversion/agent performance; revenue/collection reports; GIS/satellite
mapping; accounting/payment/SMS/email/WhatsApp integrations.

## Platform billing (parallel to the phases above)

SaaS subscription billing ([16](16-billing-and-subscriptions.md)) is an
operational concern for running the platform as a business, not a phase
in the product roadmap above — it can and should move independently.
**Status**: schema (`subscription_plans`, `organization_subscriptions`,
`billing_invoices`, `billing_webhook_events`) and a working, signature-
verified Paystack webhook receiver exist
(`crates/backend/src/paystack.rs`). Not built: any plan-selection UI,
the org sign-up flow that creates the first `organizations` row, or
enforcement of subscription status against feature access.

## Sequencing principle

Manual interactive map creation and manual payment capture ship first, as
reliable operational systems; AI-assisted plan conversion and payment-
integration automation layer on afterward, once the manual path is proven
and there's real data to validate the automation against.
