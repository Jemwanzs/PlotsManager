# 01 — Product Overview

## What this is

Real Estate Manager is a multi-tenant platform for land-project and plot
sales companies (initially modelled on a Kenyan "plots" business), built to
replace an Excel/VBA workbook with a real multi-user, auditable system.

It is built around three connected layers:

1. **Land project and inventory management** — organisations, projects,
   plots, and their statuses.
2. **Interactive project maps** — scanned/PDF land plans turned into
   clickable, colour-coded digital plot maps.
3. **Sales, booking, payment, approval, and title-transfer workflows** —
   from lead to fully-paid, transferred title.

## Why

The existing Excel/VBA workbook (`legacy-excel/PlotsManager/PPP_v.01.Xls.xlsm`)
works for a single user at a time, has no real access control, no audit
trail, and cannot represent an interactive map. It is the functional
blueprint for this system, not the destination — see
[02](02-existing-vba-system-analysis.md).

## Product pillars

- **Multi-tenant**: each organisation's projects and data are fully
  isolated (see [10](10-database-and-security-design.md)).
- **Configurable, not hardcoded**: numbering, statuses, pricing rules,
  approval chains, and notification templates are organisation-level
  settings, not code.
- **N-level approvals** on every sensitive action (see
  [09](09-approval-workflows.md)).
- **Full audit trail**: no silent edits or deletes on financial or
  ownership-affecting records.
- **Manual-first, AI-assisted later**: interactive maps and payment capture
  start as reliable manual workflows; automation (AI plan conversion,
  payment-gateway integration) layers on top once the manual path is
  trustworthy. See [14](14-development-roadmap.md).

## Infrastructure

Leptos frontend on **Vercel** (static WASM build), **Supabase** for
Postgres + Auth + Storage (the frontend talks to it directly; multi-
tenancy is Postgres Row-Level Security, not app code), a thin Rust
`services` crate for what Supabase can't do (Paystack webhooks, PDF/
schedule generation), and **Paystack** for the platform's own SaaS
subscription billing. See [10](10-database-and-security-design.md),
[12](12-api-and-integration-design.md), and
[16](16-billing-and-subscriptions.md).

## Out of scope (for now)

- Legal/registry integration — the platform's plot map and records are the
  *operational* source of truth, never the *legal* one. Title registries
  remain authoritative.
- Automatic payment reconciliation (Phase D — see roadmap).
- Satellite/GIS mapping (later phase; v1 overlays polygons on the uploaded
  plan image).
