# 14 — Development Roadmap

Two roadmaps from the original scoping conversation are merged here: the
platform-wide phases (1–7) and the payments-specific delivery sequence
(A–D), which nests inside phases 5–7.

## Current status (2026-08-26)

Infrastructure decided and scaffolded: **Supabase** (Postgres + Auth +
Storage, with the Leptos frontend calling it directly and multi-tenancy
enforced by Row-Level Security — see
[10](10-database-and-security-design.md)), **Vercel** for the frontend's
static deploy, and **Paystack** for the platform's own SaaS subscription
billing ([16](16-billing-and-subscriptions.md)). The Cargo workspace is
`domain` (shared types, now including billing), `services` (a thin Axum
service for Paystack webhooks — no longer a general CRUD backend, that
role moved to Supabase PostgREST), and `frontend` (Leptos CSR shell with
a working Supabase Auth/PostgREST client). `supabase/migrations/` holds
the schema and RLS policies. **No UI screens are built yet** — no
sign-up/login flow, no map UI, no approval engine, and the `services`
crate isn't deployed anywhere (no Rust-friendly host chosen).

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
**Status: infrastructure decided (Supabase/Vercel), schema + RLS policies
+ domain types scaffolded, frontend has a working Supabase Auth/PostgREST
client module. Not yet built: any actual sign-up/login UI, org creation
flow, numbering config, document storage wiring, or a deployed home for
the `services` crate.**

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

**Status: domain types and schema for Plot Loan Accounts/repayment
schedules/payments exist; workflow logic not implemented.**

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
(`crates/services/src/paystack.rs`). Not built: any plan-selection UI,
the org sign-up flow that creates the first `organizations` row, or
enforcement of subscription status against feature access.

## Sequencing principle

Manual interactive map creation and manual payment capture ship first, as
reliable operational systems; AI-assisted plan conversion and payment-
integration automation layer on afterward, once the manual path is proven
and there's real data to validate the automation against.
