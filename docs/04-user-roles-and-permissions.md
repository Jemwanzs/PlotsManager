# 04 — User Roles and Permissions

## Access model

Access is the combination of:

- **Role-based access** — permissions attached to a role, assignable per user.
- **Project-level access** — a role/user can be scoped to specific projects.
- **Record ownership** — e.g. an agent's own leads/bookings/customers.
- **Branch or region access**.
- **Field-level restrictions** — e.g. hide cost price/margin even where a
  role can otherwise see a plot.
- **Action-level rights** — viewing a record and acting on it (approve,
  reverse, discount) are separate grants.
- **Temporary delegated access** — time-boxed, revocable, audited.
- **Full audit trails** on every grant, use, and revocation.

This is implemented as `roles` (organisation-defined, permission list) +
`role_assignments` (user × role × optional project/branch scope) in
[10](10-database-and-security-design.md#schema); enforcement is
application-layer for v1 (row filtering by `organization_id` +
scope-checked queries), not Postgres RLS.

## Default sales-agent / marketer visibility

**Can see:**
- Active projects assigned to them
- Available plots, published selling prices, sizes, approved descriptions
- Interactive plot maps
- Approved marketing materials
- Their own leads, bookings, and customers

**Cannot see by default (grantable per role):**
- Sold plots
- Other agents' customer details
- Purchase cost or profit margin
- Minimum internal price
- Legal disputes / management notes
- Other agents' commissions
- Sensitive title documents

An administrator can grant selected users visibility into sold, reserved,
blocked, or transferred plots when needed, without changing the default
for everyone else.

## Other implied roles

- Organisation admin — configures numbering, pricing, workflows, roles.
- Project manager — owns a project's plots, team assignment, map
  publishing approvals.
- Sales/collections manager — approval authority over discounts,
  reservations, restructures, cancellations within their scope.
- Finance/accounts officer — payment capture, verification, statement
  issuance.
- Auditor — read-only, cross-project, full visibility including audit log.
- Staff/agent (daily activity) — the legacy system tracks per-staff daily
  productivity (site visits, sales, leads); if the Daily Activity Report
  in [11](11-reports-and-analytics.md) is carried forward, this role needs
  no special access beyond submitting their own entries.

## Legacy reality (see [02](02-existing-vba-system-analysis.md))

None of the above exists today. The legacy system has exactly one
authorization check in its entire codebase — a hardcoded
`username = "Admin"` string comparison gating a single screen (user
management). Every other screen, and every field on it (cost price,
minimum price, margins included), is visible to anyone who can log in
with any of the five hardcoded accounts. There is no per-project,
per-branch, or field-level restriction anywhere, and no concept of
"an agent's own customers" — all data is globally visible. This model is
being introduced fresh, not extended from precedent.
