use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    /// Direct Postgres connection to the Supabase project, using the
    /// `postgres` (or another superuser-equivalent) role — this bypasses
    /// Row-Level Security by Postgres role membership, not an API key, so
    /// this service can write billing/webhook state the frontend never
    /// gets direct write access to.
    pub db: PgPool,
    pub paystack_secret_key: String,
}
