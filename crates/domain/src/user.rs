use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
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
