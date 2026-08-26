# Documentation Index

Product and technical specification for Real Estate Manager, derived from
the original scoping conversation and the legacy Excel/VBA workbook
(`legacy-excel/`, local-only).

| Doc | Status |
|---|---|
| [01 Product Overview](01-product-overview.md) | drafted |
| [02 Existing VBA System Analysis](02-existing-vba-system-analysis.md) | **complete** — 58 modules exported and analyzed; gap-analysis table drives updates below |
| [03 Functional Requirements](03-functional-requirements.md) | drafted |
| [04 User Roles and Permissions](04-user-roles-and-permissions.md) | drafted |
| [05 Project and Plot Management](05-project-and-plot-management.md) | drafted |
| [06 Interactive Map Engine](06-interactive-map-engine.md) | drafted |
| [07 Sales and Booking Workflows](07-sales-and-booking-workflows.md) | drafted |
| [08 Payments and Receipting](08-payments-and-receipting.md) | drafted |
| [09 Approval Workflows](09-approval-workflows.md) | drafted |
| [10 Database and Security Design](10-database-and-security-design.md) | updated for Supabase — schema + RLS in `supabase/migrations/` |
| [11 Reports and Analytics](11-reports-and-analytics.md) | drafted |
| [12 API and Integration Design](12-api-and-integration-design.md) | updated for Supabase BaaS architecture |
| [13 Data Migration Plan](13-data-migration-plan.md) | drafted |
| [14 Development Roadmap](14-development-roadmap.md) | drafted, tracks current build status |
| [15 Testing and Acceptance Criteria](15-testing-and-acceptance-criteria.md) | drafted |
| [16 Billing and Subscriptions](16-billing-and-subscriptions.md) | drafted — Paystack SaaS billing, schema + webhook receiver built |

"Drafted" means the requirements are captured from the original spec
conversation; docs 03–15 have since been cross-checked against the actual
legacy system (doc 02) and annotated with "Legacy reality" notes wherever
the real workbook confirmed, contradicted, or added to the original plan.
None of this has been validated against a real client yet.
