# 14 — Development Roadmap

Two roadmaps from the original scoping conversation are merged here: the
platform-wide phases (1–7) and the payments-specific delivery sequence
(A–D), which nests inside phases 5–7.

## Current status (2026-08-26)

Workspace scaffolded: Cargo workspace with `domain`, `api` (Axum + sqlx,
health check + first real endpoint), `frontend` (Leptos CSR shell) crates;
initial Postgres schema covering organisations/projects/plots/customers/
sales/loan accounts/payments/audit log; docs directory. **Nothing beyond
scaffolding is implemented yet** — no auth, no real CRUD beyond
`GET /api/v1/organizations`, no map UI, no approval engine.

## Phase 1 — Discovery and Legacy Analysis
Analyse the Excel/VBA system, extract business rules, document current
workflows, identify migration requirements, produce full specs.
**Status: blocked on VBA export** — see [02](02-existing-vba-system-analysis.md).

## Phase 2 — Platform Foundation
Multi-tenant architecture; organisation settings; users, roles,
permissions; projects and plot register; documents and audit logs;
configurable numbering.
**Status: schema and domain types scaffolded; no auth, no numbering
config, no document storage yet.**

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

## Sequencing principle

Manual interactive map creation and manual payment capture ship first, as
reliable operational systems; AI-assisted plan conversion and payment-
integration automation layer on afterward, once the manual path is proven
and there's real data to validate the automation against.
