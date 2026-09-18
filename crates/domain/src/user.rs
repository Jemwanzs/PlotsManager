use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mirrors the `users` table (`database/migrations/0001_init.sql`).
/// Never carries `password_hash` — that field exists only in Postgres and
/// in `crates/backend/src/auth.rs`'s hashing/verification code, never in
/// a struct that could end up serialized into an API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub full_name: String,
    pub email: String,
    pub is_active: bool,
    /// The platform operator's own account, not a tenant permission —
    /// see `database/migrations/0004_platform_ownership.sql` and
    /// `crates/backend/src/routes/platform.rs`. Governs access to the
    /// cross-tenant `/api/v1/platform/*` endpoints; unrelated to the
    /// per-organization `roles`/`role_assignments` RBAC below.
    pub is_platform_owner: bool,
    pub created_at: DateTime<Utc>,
}

/// Roles are organisation-defined records in the database (role name +
/// a set of permission strings); this is only the shape of the assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub project_id: Option<Uuid>,
    pub branch_id: Option<Uuid>,
}

/// Mirrors the `roles` table — an organization-defined name plus the set
/// of `domain::permissions` strings it grants (`["*"]` for the
/// auto-provisioned signup "Admin" role). `assigned_user_count` is
/// listing-only context (how many users currently hold this role), not
/// a column on the row itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub permissions: Vec<String>,
    pub assigned_user_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleInput {
    pub name: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleInput {
    pub name: String,
    pub permissions: Vec<String>,
}
