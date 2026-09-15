-- Platform-owner layer: one account (the platform operator, not a
-- customer tenant) that can see and manage every organization —
-- listing tenants, their users, billing/subscription state, access
-- history, and deactivating a tenant. Distinct from the per-organization
-- `roles`/`role_assignments` RBAC in 0001_init.sql, which is scoped to a
-- single organization and can't express "see across all of them" —
-- platform ownership is a flag checked by the backend
-- (crates/backend/src/extractors.rs), not a tenant-scoped role.

alter table users add column is_platform_owner boolean not null default false;

-- A deactivated tenant's users can no longer log in (enforced in
-- crates/backend/src/routes/auth.rs) — this is the platform owner's
-- "deactivate such tenants" lever. Kept as a simple two-state flag
-- rather than folding into organization_subscriptions.status, since
-- deactivation is an administrative action independent of billing state
-- (a tenant can be deactivated for cause while still mid-subscription).
alter table organizations add column status text not null default 'active'
    check (status in ('active', 'deactivated'));
