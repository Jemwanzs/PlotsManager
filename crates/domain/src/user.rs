use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mirrors the `profiles` table, which extends Supabase Auth's
/// `auth.users` (id only) with the organization/branch scoping the rest of
/// the schema keys off. Supabase owns credentials, sessions, and password
/// resets — this crate never models a password.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub full_name: String,
    pub email: String,
    pub is_active: bool,
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
