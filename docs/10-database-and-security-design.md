# 10 — Database and Security Design

## Multi-tenancy

Every tenant-scoped table carries `organization_id` directly, or
transitively through a foreign key chain that terminates in one (e.g.
`plots.project_id → projects.organization_id`). v1 enforces isolation at
the application/query layer (every query is scoped by the authenticated
user's `organization_id`); Postgres Row-Level Security policies are a
natural hardening step once the query patterns stabilise, not a v1
requirement.

## Schema (current)

Implemented in
[`crates/api/migrations/0001_init.sql`](../crates/api/migrations/0001_init.sql):

- `organizations`, `branches`, `roles`, `users`, `role_assignments`
- `projects`, `project_map_versions`, `plot_status_config`, `plots`
- `customers`
- `plot_sales`, `plot_loan_accounts`, `repayment_schedule_entries`,
  `payments`
- `audit_log`

This covers Phase 2 (foundation) and the core of Phase 5/Phase A (sales,
Plot Loan Accounts, payments) from the roadmap. Not yet modelled:
approval-workflow definitions/instances, notification templates, document
storage/versioning, commission records — added as those phases start.

## Security and audit controls

- Project-, branch-, role-, and record-level permissions (see
  [04](04-user-roles-and-permissions.md)), enforced separately for
  capture, verification, approval, reversal, and reporting actions.
- **No deletion of posted financial transactions.** Corrections are a
  reversal + replacement, both rows kept.
- Complete before/after state captured for sensitive changes
  (`audit_log.before_state` / `after_state`, JSONB).
- Duplicate-prevention on plot numbers, account numbers, and payment
  references (unique constraints in the schema, not just app-level checks).
- Mandatory reasons for overrides, reversals, waivers, and backdated
  entries (enforced at the API layer where the action is performed).
- Sensitive-field masking (e.g. cost price, minimum price) applied at the
  API response layer based on role, not by relying on the frontend to hide
  fields.
- Session/login and suspicious-activity logging (to be added alongside
  auth implementation).
- Locked accounting periods, where configured, block backdated posting.

## Money and identifiers

- All monetary fields are `numeric` (Postgres) / `rust_decimal::Decimal`
  (Rust) — never floating point.
- Every plot, sale, and loan account has a system-generated UUID as its
  durable identity, independent of any external reference (title number,
  receipt number) that may not exist yet or may change.

## Open questions for a later pass

- Auth mechanism (session cookies vs. JWT) and where MFA fits.
- Whether Postgres RLS is worth adding once tenant count grows, vs. the
  ongoing cost of the app-layer scoping being airtight.
- File/document storage backend (local disk vs. S3-compatible object
  storage) for uploaded plans, proofs of payment, and generated PDFs.
