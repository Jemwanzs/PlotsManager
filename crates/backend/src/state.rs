use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    /// Direct Postgres connection to Railway Postgres. In production this
    /// should be a role with BYPASSRLS (system/admin operations — running
    /// migrations, Paystack webhook writes — aren't tied to one tenant's
    /// session the way ordinary request handling is); see
    /// docs/10-database-and-security-design.md for the still-open task of
    /// provisioning a separate, non-privileged role for ordinary
    /// request-scoped queries once those exist.
    pub db: PgPool,
    pub paystack_secret_key: String,
    /// Signs and verifies session tokens (`crates/backend/src/auth.rs`).
    pub jwt_secret: String,
}
