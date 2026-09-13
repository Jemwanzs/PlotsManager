use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

// Complete and tested (see the module's own tests), but not yet wired to
// any HTTP route — signup/login handlers are later roadmap work
// (docs/14-development-roadmap.md). Silence dead_code until then rather
// than leaving real warnings that would mask new ones.
#[allow(dead_code)]
mod auth;
mod paystack;
mod state;

use state::AppState;

/// Real Estate Manager's backend: the only thing that talks to Postgres.
/// The frontend never connects to the database directly — every request
/// goes through this API, which is the primary enforcement point for
/// authentication, authorization, and tenant isolation (Row-Level Security
/// in the schema is defense-in-depth behind this, not a substitute for
/// it). See docs/10-database-and-security-design.md and
/// docs/12-api-and-integration-design.md.
///
/// Today this only exposes a health check and the Paystack webhook
/// receiver — the frontend is being built against mock data first
/// (docs/14-development-roadmap.md), so most of this crate's eventual job
/// (CRUD endpoints, auth handlers using `auth.rs`) doesn't exist yet.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "backend=debug,tower_http=debug".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set (see .env.example)");
    let paystack_secret_key =
        std::env::var("PAYSTACK_SECRET_KEY").expect("PAYSTACK_SECRET_KEY must be set");

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    tracing::info!("running database migrations");
    sqlx::migrate!("../../database/migrations").run(&db).await?;

    let state = AppState {
        db,
        paystack_secret_key,
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(paystack::router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Railway injects PORT and expects the app to bind to it; BIND_ADDR
    // remains for local dev where PORT usually isn't set.
    let addr: SocketAddr = match std::env::var("PORT") {
        Ok(port) => format!("0.0.0.0:{port}").parse()?,
        Err(_) => std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()?,
    };
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}
