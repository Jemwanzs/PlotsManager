use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod auth;
mod error;
mod extractors;
mod paystack;
mod pg_enum;
mod routes;
mod state;
mod tenant_gate;

use state::AppState;

/// Real Estate Manager's backend: the only thing that talks to Postgres.
/// The frontend never connects to the database directly — every request
/// goes through this API, which is the primary enforcement point for
/// authentication, authorization, and tenant isolation (Row-Level Security
/// in the schema is defense-in-depth behind this, not a substitute for
/// it). See docs/10-database-and-security-design.md and
/// docs/12-api-and-integration-design.md.
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
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    tracing::info!("running database migrations");
    sqlx::migrate!("../../database/migrations").run(&db).await?;

    let state = AppState {
        db,
        paystack_secret_key,
        jwt_secret,
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(paystack::router())
        .merge(routes::router())
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
