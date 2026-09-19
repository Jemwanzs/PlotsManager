//! The permission catalog — every distinct capability a role can be
//! granted. Deliberately small and specific to what the Settings →
//! Security governance layer actually gates today (user management,
//! role management, access/audit visibility, approval configuration and
//! approval action) rather than a speculative full CRUD-per-module
//! matrix; new permissions get added here as the modules that need them
//! are built, not invented ahead of time.
//!
//! A role's `permissions` (`roles.permissions` jsonb, `database/
//! migrations/0001_init.sql`) is a plain `Vec<String>` of these constants
//! — `"*"` (set on the auto-created signup "Admin" role) is a wildcard
//! meaning every permission, so existing organizations keep working
//! unchanged after this catalog was introduced.

pub const PERM_MANAGE_USERS: &str = "users:manage";
pub const PERM_MANAGE_ROLES: &str = "roles:manage";
pub const PERM_MANAGE_BRANCHES: &str = "branches:manage";
pub const PERM_RESET_PASSWORD: &str = "users:reset_password";
pub const PERM_VIEW_ACCESS_LOGS: &str = "security:view_access_logs";
pub const PERM_MANAGE_SESSIONS: &str = "security:manage_sessions";
pub const PERM_VIEW_AUDIT_LOGS: &str = "security:view_audit_logs";
pub const PERM_CONFIGURE_ACCESS_POLICIES: &str = "security:configure_access_policies";
pub const PERM_CONFIGURE_APPROVALS: &str = "security:configure_approvals";
pub const PERM_APPROVE_TRANSACTIONS: &str = "approvals:approve";

/// `(permission, human label)` — drives the Roles & Permissions editor's
/// checkbox list so the catalog only has to change in one place.
pub const ALL_PERMISSIONS: &[(&str, &str)] = &[
    (PERM_MANAGE_USERS, "Manage users"),
    (PERM_MANAGE_ROLES, "Manage roles"),
    (PERM_MANAGE_BRANCHES, "Manage branches"),
    (PERM_RESET_PASSWORD, "Reset user passwords"),
    (PERM_VIEW_ACCESS_LOGS, "View access logs"),
    (PERM_MANAGE_SESSIONS, "Manage sessions"),
    (PERM_VIEW_AUDIT_LOGS, "View audit logs"),
    (PERM_CONFIGURE_ACCESS_POLICIES, "Configure access policies"),
    (PERM_CONFIGURE_APPROVALS, "Configure approval workflows"),
    (PERM_APPROVE_TRANSACTIONS, "Approve transactions"),
];

/// The wildcard every auto-provisioned org-admin role carries.
pub const PERM_WILDCARD: &str = "*";
