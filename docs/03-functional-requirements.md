# 03 — Functional Requirements

High-level functional scope. Each area is expanded in its own doc.

## Organisation setup ([05](05-project-and-plot-management.md))
Name/code, branches, regions, currency/tax rules, numbering schemes for
projects and plots, pricing rules, booking/reservation rules, commission
rules, payment plans, approval workflows, roles/access rights,
notification templates, document templates, security/audit settings.

## Land projects ([05](05-project-and-plot-management.md))
Name, unique code, location, GPS/boundary, original title/parcel number,
total size + unit, purchase/ownership details, surveyor/legal info,
status, plot count, roads/utilities/public areas, project plan document,
pricing/payment plans, supporting documents, assigned manager and sales
team.

## Plots ([05](05-project-and-plot-management.md))
Internal plot ID (immutable, system-generated regardless of whether a
title exists yet), project plot number, title/parcel number where
available, tenant-issued unique code, size, asking price, minimum
acceptable price, status, map polygon/coordinates, road frontage/access,
amenities, assigned customer, booking/sales history, payment status,
transfer status, documents/approvals.

## Interactive maps ([06](06-interactive-map-engine.md))
Upload scanned/PDF plans, detect and draw plot polygons, link polygons to
plot records, colour-code by status, click/hover for plot detail, filter
by size/price/section, versioned and approval-gated publishing.

## Sales and booking ([07](07-sales-and-booking-workflows.md))
Lead → selection → hold → reservation → booking → approval → sale →
transfer, with a configurable plot status model and access rules for
marketers vs. managers.

## Payments and Lipa Pole Pole ([08](08-payments-and-receipting.md))
Full cash sales and interest-free/interest-bearing instalment sales, each
backed by a Plot Loan Account with a generated, recalculable repayment
schedule; manual payment capture with verification and allocation;
statements, receipts, and settlement documents; arrears/collections
management.

## Approvals ([09](09-approval-workflows.md))
Configurable N-level approval chains covering project/plot setup,
pricing/discounts, reservations, payment verification/reversal,
restructures, cancellations, and title-transfer initiation.

## Access control ([04](04-user-roles-and-permissions.md))
Role-based, project-level, branch/region-level, record-ownership, and
field-level access, with temporary delegation and full audit trails.

## Reporting and analytics ([11](11-reports-and-analytics.md))
Executive, project, finance/collections, and sales dashboards; a report
library covering inventory, financial, arrears, and audit reporting, with
PDF/Excel/CSV export.

## Non-functional
- Multi-tenant data isolation is a hard requirement, not a convention (see
  [10](10-database-and-security-design.md)).
- No deletion of posted financial transactions — corrections are
  reversal + replacement, always audited.
- Every published interactive map version is immutable once approved;
  edits create a new draft version.
