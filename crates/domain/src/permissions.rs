//! The permission registry — the single source of truth for every
//! distinct capability a tenant role can be granted, organized as
//! Module -> Feature -> Action rather than a flat list, so the
//! Roles & Permissions editor can render it hierarchically
//! (`crates/frontend/src/pages/roles.rs`) and so adding a new
//! capability later is "add one `PermissionDef` to `ALL_PERMISSIONS`",
//! never a UI redesign.
//!
//! Grounded in the actual route surface as of this registry's
//! introduction (every `crates/backend/src/routes/*.rs` handler was
//! inventoried first) — it does not invent actions that don't exist
//! yet (e.g. there's no "cancel a sale" or "delete a plot" route
//! today, so no such permission key exists either). When a route like
//! that is built, add its key here alongside it.
//!
//! `roles.permissions` (`database/migrations/0001_init.sql`, jsonb) is
//! a plain `Vec<String>` of these `key` values — `"*"` (set on the
//! auto-created signup "Admin" role) is a wildcard meaning every
//! permission. Existing keys' string VALUES are never renamed once
//! shipped (only added to) — they're what's actually stored in every
//! tenant's `roles` rows, and changing one would silently revoke it
//! from every role that has it.
//!
//! Not part of this registry by design: `Platform` (cross-tenant admin,
//! `crates/backend/src/routes/platform.rs`) is gated by the separate
//! `AuthUser.is_platform_owner` boolean, not a permissions string, and
//! stays that way — folding it in here would let a tenant admin grant
//! a tenant role platform-owner capabilities, which is exactly what it
//! must never be able to do.

/// One assignable capability, as authored in the registry below —
/// `&'static str` so the ~45-entry literal list stays a plain `const`
/// array with no per-entry allocation. Not itself sent over the wire
/// (see `PermissionDef`, its owned counterpart) since `&'static str`
/// can't round-trip through `Deserialize`.
#[derive(Debug, Clone, Copy)]
struct StaticPermissionDef {
    key: &'static str,
    label: &'static str,
    module: &'static str,
    feature: &'static str,
    sensitive: bool,
}

/// The wire/owned form of `StaticPermissionDef` — what `GET
/// /api/v1/roles/permissions` actually returns and what the frontend
/// deserializes into. `module`/`feature` group it for display; `key`
/// is what's actually stored on a role and checked by
/// `AuthUser::has_permission`. `sensitive` flags actions with outsized
/// blast radius (deletion, money movement, security/user
/// administration) so the editor can visually call them out rather
/// than let them blend into a long checkbox list.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionDef {
    pub key: String,
    pub label: String,
    pub module: String,
    pub feature: String,
    pub sensitive: bool,
}

// ---------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------
pub const PERM_DASHBOARD_VIEW: &str = "dashboard:view";

// ---------------------------------------------------------------
// Projects
// ---------------------------------------------------------------
pub const PERM_PROJECTS_VIEW: &str = "projects:view";
pub const PERM_PROJECTS_CREATE: &str = "projects:create";

// ---------------------------------------------------------------
// Plots — three features: core CRUD, the interactive map, and the
// reserve/book transaction flow (three separate route files today:
// projects.rs, project_map.rs, sales.rs).
// ---------------------------------------------------------------
pub const PERM_PLOTS_VIEW: &str = "plots:view";
pub const PERM_PLOTS_CREATE: &str = "plots:create";
pub const PERM_PLOTS_EDIT: &str = "plots:edit";
pub const PERM_PLOTS_BULK_IMPORT: &str = "plots:bulk_import";
pub const PERM_PLOTS_MAP_VIEW: &str = "plots:map.view";
pub const PERM_PLOTS_MAP_UPLOAD: &str = "plots:map.upload";
pub const PERM_PLOTS_MAP_EDIT_BOUNDARIES: &str = "plots:map.edit_boundaries";
pub const PERM_PLOTS_MAP_LINK: &str = "plots:map.link";
pub const PERM_PLOTS_TRANSACTIONS_CREATE: &str = "plots:transactions.create";
pub const PERM_PLOTS_TRANSACTIONS_BULK_IMPORT: &str = "plots:transactions.bulk_import";

// ---------------------------------------------------------------
// Customers — core CRUD plus the lead/pipeline sub-feature
// (`update_lead` in customers.rs).
// ---------------------------------------------------------------
pub const PERM_CUSTOMERS_VIEW: &str = "customers:view";
pub const PERM_CUSTOMERS_CREATE: &str = "customers:create";
/// Editing a customer's own profile fields (KYC details, next of kin,
/// contact info) — new alongside `update_customer`, which didn't exist
/// until now (a customer could be created and viewed, but never
/// edited). Distinct from `customers:leads.update` (pipeline
/// stage/notes/follow-up), which changes far more often and doesn't
/// need the same gate.
pub const PERM_CUSTOMERS_EDIT: &str = "customers:edit";
pub const PERM_CUSTOMERS_BULK_IMPORT: &str = "customers:bulk_import";
pub const PERM_CUSTOMERS_LEADS_UPDATE: &str = "customers:leads.update";

// ---------------------------------------------------------------
// Quotes
// ---------------------------------------------------------------
pub const PERM_QUOTES_VIEW: &str = "quotes:view";
pub const PERM_QUOTES_CREATE: &str = "quotes:create";
pub const PERM_QUOTES_SEND: &str = "quotes:send";
/// Converts a quotation into a real sale (same effect as
/// `plots:transactions.create`) — deliberately as sensitive as that.
pub const PERM_QUOTES_ACCEPT: &str = "quotes:accept";
pub const PERM_QUOTES_REJECT: &str = "quotes:reject";

// ---------------------------------------------------------------
// Approvals (below-minimum-price gate)
// ---------------------------------------------------------------
pub const PERM_APPROVALS_VIEW: &str = "approvals:view";
pub const PERM_APPROVE_TRANSACTIONS: &str = "approvals:approve";

// ---------------------------------------------------------------
// Finance — loan accounts (read), recording a payment, and posting a
// manual interest/penalty charge against one.
// ---------------------------------------------------------------
pub const PERM_FINANCE_VIEW: &str = "finance:view";
pub const PERM_PAYMENTS_RECORD: &str = "payments:record";
/// Posting a manual interest or penalty charge
/// (`crates/backend/src/routes/loan_accounts.rs::post_charge`) —
/// distinct from `payments:record` since it *increases* what a
/// customer owes rather than reduces it; a role trusted to record
/// what customers paid isn't automatically trusted to add charges.
pub const PERM_FINANCE_POST_CHARGES: &str = "finance:post_charges";
/// Reversing a payment/charge or waiving outstanding interest/penalty
/// (`routes/loan_accounts.rs::reverse_entry`/`post_waiver`) — the
/// undo/forgive counterpart to `payments:record`/`finance:post_charges`,
/// kept as its own key rather than folded into either since a role
/// trusted to record or charge isn't automatically trusted to reverse
/// or forgive what's already posted.
pub const PERM_FINANCE_REVERSE: &str = "finance:reverse";

// ---------------------------------------------------------------
// Reports
// ---------------------------------------------------------------
pub const PERM_REPORTS_SALES: &str = "reports:sales";
pub const PERM_REPORTS_INVENTORY: &str = "reports:inventory";
/// Individual agent performance/conversion stats — kept distinct from
/// the other two reports since it's compensation-adjacent and not
/// every role that can see aggregate sales/inventory should see it.
pub const PERM_REPORTS_AGENT_PERFORMANCE: &str = "reports:agent_performance";

// ---------------------------------------------------------------
// Users, Branches, Roles — unchanged from the original flat catalog;
// these string values are already live in production `roles` rows, so
// they're kept byte-for-byte identical, just re-homed into the
// hierarchical registry below.
// ---------------------------------------------------------------
pub const PERM_MANAGE_USERS: &str = "users:manage";
pub const PERM_RESET_PASSWORD: &str = "users:reset_password";
pub const PERM_MANAGE_BRANCHES: &str = "branches:manage";
/// New: `list_branches` had no read-side check while every branch
/// mutation did — added for symmetry with the read/write split used
/// everywhere else in this registry.
pub const PERM_BRANCHES_VIEW: &str = "branches:view";
pub const PERM_MANAGE_ROLES: &str = "roles:manage";

// ---------------------------------------------------------------
// Security — unchanged values; some are not enforced by any route yet
// (reserved for the Settings -> Security phase of the tenant/billing
// spec: location policy, session/access monitoring) but stay in the
// registry now so that phase only has to wire enforcement, not invent
// new permission keys under a live role migration.
// ---------------------------------------------------------------
pub const PERM_VIEW_ACCESS_LOGS: &str = "security:view_access_logs";
pub const PERM_MANAGE_SESSIONS: &str = "security:manage_sessions";
pub const PERM_VIEW_AUDIT_LOGS: &str = "security:view_audit_logs";
pub const PERM_CONFIGURE_ACCESS_POLICIES: &str = "security:configure_access_policies";
pub const PERM_CONFIGURE_APPROVALS: &str = "security:configure_approvals";

// ---------------------------------------------------------------
// Settings (organization-wide configuration: currency, timezone,
// numbering) — new; today ANY authenticated org member can change
// this for the whole tenant (`routes/settings.rs`'s own doc comment
// flags it as a known, deliberate gap). Highest-priority real
// enforcement target in this registry's first rollout.
// ---------------------------------------------------------------
pub const PERM_SETTINGS_MANAGE_ORGANIZATION: &str = "settings:manage_organization";

/// The wildcard every auto-provisioned org-admin role carries.
pub const PERM_WILDCARD: &str = "*";

/// The full registry, grouped for the editor UI. Order here is display
/// order (module, then feature, then action) — group consecutive
/// entries by `(module, feature)` to render sections; don't re-sort by
/// key or label, the grouping relies on entries for the same
/// module/feature being adjacent.
const REGISTRY: &[StaticPermissionDef] = &[
    StaticPermissionDef { key: PERM_DASHBOARD_VIEW, label: "View dashboard", module: "Dashboard", feature: "Analytics", sensitive: false },

    StaticPermissionDef { key: PERM_PROJECTS_VIEW, label: "View projects", module: "Projects", feature: "Project management", sensitive: false },
    StaticPermissionDef { key: PERM_PROJECTS_CREATE, label: "Create projects", module: "Projects", feature: "Project management", sensitive: false },

    StaticPermissionDef { key: PERM_PLOTS_VIEW, label: "View plots", module: "Plots", feature: "Plot management", sensitive: false },
    StaticPermissionDef { key: PERM_PLOTS_CREATE, label: "Create plots", module: "Plots", feature: "Plot management", sensitive: false },
    StaticPermissionDef { key: PERM_PLOTS_EDIT, label: "Edit plots", module: "Plots", feature: "Plot management", sensitive: false },
    StaticPermissionDef { key: PERM_PLOTS_BULK_IMPORT, label: "Bulk import plots", module: "Plots", feature: "Plot management", sensitive: true },
    StaticPermissionDef { key: PERM_PLOTS_MAP_VIEW, label: "View plot map", module: "Plots", feature: "Plot map", sensitive: false },
    StaticPermissionDef { key: PERM_PLOTS_MAP_UPLOAD, label: "Upload site plan image", module: "Plots", feature: "Plot map", sensitive: false },
    StaticPermissionDef { key: PERM_PLOTS_MAP_EDIT_BOUNDARIES, label: "Draw/edit plot boundaries", module: "Plots", feature: "Plot map", sensitive: false },
    StaticPermissionDef { key: PERM_PLOTS_MAP_LINK, label: "Link/unlink plots to map shapes", module: "Plots", feature: "Plot map", sensitive: false },
    StaticPermissionDef { key: PERM_PLOTS_TRANSACTIONS_CREATE, label: "Reserve/book a plot", module: "Plots", feature: "Plot transactions", sensitive: true },
    StaticPermissionDef { key: PERM_PLOTS_TRANSACTIONS_BULK_IMPORT, label: "Bulk import historical sales", module: "Plots", feature: "Plot transactions", sensitive: true },

    StaticPermissionDef { key: PERM_CUSTOMERS_VIEW, label: "View customers", module: "Customers", feature: "Customer management", sensitive: false },
    StaticPermissionDef { key: PERM_CUSTOMERS_CREATE, label: "Create customers", module: "Customers", feature: "Customer management", sensitive: false },
    StaticPermissionDef { key: PERM_CUSTOMERS_EDIT, label: "Edit customer profiles", module: "Customers", feature: "Customer management", sensitive: false },
    StaticPermissionDef { key: PERM_CUSTOMERS_BULK_IMPORT, label: "Bulk import customers", module: "Customers", feature: "Customer management", sensitive: true },
    StaticPermissionDef { key: PERM_CUSTOMERS_LEADS_UPDATE, label: "Update lead pipeline stage", module: "Customers", feature: "Leads", sensitive: false },

    StaticPermissionDef { key: PERM_QUOTES_VIEW, label: "View quotations", module: "Quotes", feature: "Quotations", sensitive: false },
    StaticPermissionDef { key: PERM_QUOTES_CREATE, label: "Create quotations", module: "Quotes", feature: "Quotations", sensitive: false },
    StaticPermissionDef { key: PERM_QUOTES_SEND, label: "Send quotations", module: "Quotes", feature: "Quotations", sensitive: false },
    StaticPermissionDef { key: PERM_QUOTES_ACCEPT, label: "Accept quotations (creates a sale)", module: "Quotes", feature: "Quotations", sensitive: true },
    StaticPermissionDef { key: PERM_QUOTES_REJECT, label: "Reject quotations", module: "Quotes", feature: "Quotations", sensitive: false },

    StaticPermissionDef { key: PERM_APPROVALS_VIEW, label: "View approval requests", module: "Approvals", feature: "Price approvals", sensitive: false },
    StaticPermissionDef { key: PERM_APPROVE_TRANSACTIONS, label: "Approve/reject transactions", module: "Approvals", feature: "Price approvals", sensitive: true },

    StaticPermissionDef { key: PERM_FINANCE_VIEW, label: "View loan accounts", module: "Finance", feature: "Loan accounts", sensitive: false },
    StaticPermissionDef { key: PERM_PAYMENTS_RECORD, label: "Record a payment", module: "Finance", feature: "Payments", sensitive: true },
    StaticPermissionDef { key: PERM_FINANCE_POST_CHARGES, label: "Post interest/penalty charges", module: "Finance", feature: "Payments", sensitive: true },
    StaticPermissionDef { key: PERM_FINANCE_REVERSE, label: "Reverse payments/charges, waive interest/penalty", module: "Finance", feature: "Payments", sensitive: true },

    StaticPermissionDef { key: PERM_REPORTS_SALES, label: "View sales report", module: "Reports", feature: "Reports", sensitive: false },
    StaticPermissionDef { key: PERM_REPORTS_INVENTORY, label: "View inventory report", module: "Reports", feature: "Reports", sensitive: false },
    StaticPermissionDef { key: PERM_REPORTS_AGENT_PERFORMANCE, label: "View agent performance report", module: "Reports", feature: "Reports", sensitive: true },

    StaticPermissionDef { key: PERM_MANAGE_USERS, label: "Manage users", module: "Users & access", feature: "Users", sensitive: true },
    StaticPermissionDef { key: PERM_RESET_PASSWORD, label: "Reset user passwords", module: "Users & access", feature: "Users", sensitive: true },
    StaticPermissionDef { key: PERM_BRANCHES_VIEW, label: "View branches", module: "Users & access", feature: "Branches", sensitive: false },
    StaticPermissionDef { key: PERM_MANAGE_BRANCHES, label: "Manage branches", module: "Users & access", feature: "Branches", sensitive: false },
    StaticPermissionDef { key: PERM_MANAGE_ROLES, label: "Manage roles & permissions", module: "Users & access", feature: "Roles", sensitive: true },

    StaticPermissionDef { key: PERM_VIEW_ACCESS_LOGS, label: "View access logs", module: "Security", feature: "Monitoring", sensitive: false },
    StaticPermissionDef { key: PERM_MANAGE_SESSIONS, label: "Manage user sessions", module: "Security", feature: "Monitoring", sensitive: true },
    StaticPermissionDef { key: PERM_VIEW_AUDIT_LOGS, label: "View audit logs", module: "Security", feature: "Monitoring", sensitive: false },
    StaticPermissionDef { key: PERM_CONFIGURE_ACCESS_POLICIES, label: "Configure access policies", module: "Security", feature: "Policies", sensitive: true },
    StaticPermissionDef { key: PERM_CONFIGURE_APPROVALS, label: "Configure approval workflows", module: "Security", feature: "Policies", sensitive: true },

    StaticPermissionDef { key: PERM_SETTINGS_MANAGE_ORGANIZATION, label: "Manage organization settings", module: "Settings", feature: "Organization", sensitive: true },
];

/// The registry as owned, wire-ready `PermissionDef`s — what
/// `GET /api/v1/roles/permissions` (`crates/backend/src/routes/
/// roles.rs`) returns and what the Roles & Permissions editor
/// (`crates/frontend/src/pages/roles.rs`) renders, grouped by
/// `module`/`feature` in registry order.
pub fn all_permissions() -> Vec<PermissionDef> {
    REGISTRY
        .iter()
        .map(|d| PermissionDef {
            key: d.key.to_string(),
            label: d.label.to_string(),
            module: d.module.to_string(),
            feature: d.feature.to_string(),
            sensitive: d.sensitive,
        })
        .collect()
}

/// Every known permission key — used to validate a role's permission
/// list on create/update (`crates/backend/src/routes/roles.rs`).
pub fn all_permission_keys() -> Vec<&'static str> {
    REGISTRY.iter().map(|d| d.key).collect()
}
