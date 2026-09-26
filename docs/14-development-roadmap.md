# 14 — Development Roadmap

Two roadmaps from the original scoping conversation are merged here: the
platform-wide phases (1–7) and the payments-specific delivery sequence
(A–D), which nests inside phases 5–7.

## Current status (2026-09-15)

**Architecture (final): Frontend → Rust API → PostgreSQL, all on
Railway** (project `c7bee255-492d-40b6-af50-30374625b279`). This project
briefly targeted Supabase + Vercel (2026-08-26 to 2026-09-13); that's been
fully refactored away — the Leptos frontend never talks to the database,
`crates/backend` (Axum) owns authentication/authorization/tenant
isolation, and Postgres lives on Railway with Row-Level Security as
defense-in-depth, not the enforcement point. See
[10](10-database-and-security-design.md) and
[12](12-api-and-integration-design.md).

The Cargo workspace is `domain` (shared types incl. billing, plus the
request/response DTOs and status-color helpers both `backend` and
`frontend` share — `api_types.rs`, `status_meta.rs`), `backend` (Axum —
health check, Paystack webhook receiver, and now real routes: login,
dashboard, projects/plots, customers, sales, loan accounts/payments, all
authenticated via JWT and scoped by `organization_id`), and `frontend`
(Leptos CSR shell with routing, a responsive app shell, and real screens
— login, dashboard, projects, project/plot detail — still built against
the in-memory mock dataset behind `crates/frontend/src/api/`; wiring it
to the now-verified real backend is the next step). `database/migrations/`
holds the schema and RLS policies, applied automatically by the backend
on boot; `database/seeds/0002_dev_demo.sql` is a dev-only dataset
mirroring the frontend mock, for backend verification against real
Postgres.

**Priority order for what's next** (per the 2026-09-13 architecture
decision, updated 2026-09-15 now that the backend is real): Frontend/API
Integration (point `crates/frontend/src/api/http.rs` at the real
backend) → External Integrations → Railway deployment (Dockerfiles for
`backend`/`frontend`, service config — not started) → Testing/Security →
Production Hardening. The least-privilege RLS-subject Postgres role
(today everything runs through one connection; RLS is real but not yet
the primary boundary for a scoped DB role) remains an explicitly
deferred hardening item, not a blocker for the above.

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
Postgres), schema + RLS policies + domain types built, and the backend
API is real and verified against Postgres** — login (Argon2 + JWT),
projects/plots (create + list + detail), customers (create + list +
detail with purchase history), all authenticated and scoped by
`organization_id`. Frontend still runs against its in-memory mock (same
interface, not yet pointed at the real backend — see the priority order
above). Not yet built: org creation/signup flow, numbering config,
document storage wiring, the least-privilege RLS-subject Postgres role
(see [10](10-database-and-security-design.md)), or Railway deployment
configs for `backend`/`frontend`. The plot register has real create
flows against mock data in the UI — new project, new plot (per-project
unique plot numbers, fixing the legacy global-uniqueness bug from
[02](02-existing-vba-system-analysis.md) §3), new customer — each with
the validation the schema itself enforces (duplicate codes/IDs rejected)
replicated in the mock and now proven identical in the real backend.**

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
**Status: frontend groundwork against mock data, backend now real and
verified.** Customers list + detail (purchase history) and a "Reserve
this plot" flow exist in the UI — picking a customer and payment mode on
an uncommitted plot creates a `PlotSale`, moves the plot to
Reserved/Booked, and the plot grid updates live; the backend implements
the same flow transactionally against Postgres (`POST /api/v1/sales`),
verified for both Full Cash and Lipa Pole Pole modes. Not built:
leads/prospects, holds vs. reservations as distinct stages, quotations/
offer letters, agent commissions, a real customer 360° view (today's
customer detail is purchase history only).

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

**Status: Phase A frontend groundwork against mock data, backend now
real and verified.** A Plot Loan Account is created automatically when a
Lipa Pole Pole sale is reserved (fixed 10% deposit / 12 monthly
instalments — no tenor/deposit picker yet), with a detail screen
(balance, instalment, deposit) and a "Record a payment" form that
updates the running balance and status (Awaiting Deposit → Active
(Partially Paid) → Fully Paid) live; the backend implements the same
account creation and payment recalculation transactionally against
Postgres (`POST /api/v1/sales`, `POST /api/v1/loan-accounts/:id/payments`),
using a real Postgres sequence for account numbers (migration 0003 —
the legacy `count(*)+1`-style numbering bug from
[02](02-existing-vba-system-analysis.md) §3 doesn't get a chance to
reappear here). Not built: a real generated repayment schedule
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

## UI/UX consistency — number/currency display, commission override clarity (2026-09-26)

Raised via user feedback (screenshots of the loan account detail page
and the project header's commission override control): repeating the
currency code beside every figure in a tight grid wastes horizontal
space and risks numbers wrapping mid-value, and "Override" as a bare
button label doesn't say what's being overridden.

**Done:**
- `format::format_amount` (bare number, no currency code) is now the
  default for every stat-card-style grid and dt/dd summary panel:
  dashboard (already the reference implementation this pattern was
  generalized from), Finance Overview (already correct), loan account
  detail's stat grid, the Sales report's stat-card row, and project
  detail's plot tiles + commercial-position panel. Each of those
  sections now shows the currency exactly once via a shared
  `.currency-note` badge (renamed from the dashboard-only
  `.dashboard-currency` it started as) instead of repeating it per
  figure.
- `.stat-card .stat-value` uses `clamp()` for responsive font-sizing
  and keeps `white-space: nowrap` (already present) so a figure never
  breaks mid-number, only ever shrinks or (via nowrap) stays intact.
- Left `format_money` (currency-inline) deliberately in two kinds of
  place: (1) tables/lists where each row is scanned independently
  (payment history, ledger entries, report rows, receipts' own line
  items) — no redundancy to remove there, each row is its own
  context; (2) printable/exportable documents (the payment receipt,
  the loan statement) — these are meant to be viewed or shared
  outside the surrounding app chrome (printed, screenshotted), where
  restating the currency per figure is the more correct convention,
  not a redundancy.
- `ProjectCommissionEditor` (project detail page) rewritten: the bare
  "Override" button is now "Override Commission Rate" (or "Edit
  Override" once one is active); the summary line reads "Default
  Commission: X% → Override Commission Rate: Y%" instead of a bare
  percentage with no reference point; a new optional
  `commission_rate_override_reason` (migration `0034`, stored on
  `projects`, cleared automatically whenever the override itself is
  cleared) is shown under the summary and captured in the edit form
  alongside a static "Applies to: this project" line (the only scope
  that exists today — no per-sale/per-agent override); "Clear
  override" is now "Remove Override / Restore Default".

**Deliberately not swept this pass** (isolated single/double money
mentions on a page, not a repeated grid — the "where appropriate"
carve-out the request itself named): customer detail, quotation
detail/list, approvals list, recent activity, the org-wide loan
accounts list. A future pass could still apply `.currency-note` there
if a page grows more than one or two money mentions.



Manual interactive map creation and manual payment capture ship first, as
reliable operational systems; AI-assisted plan conversion and payment-
integration automation layer on afterward, once the manual path is proven
and there's real data to validate the automation against.
