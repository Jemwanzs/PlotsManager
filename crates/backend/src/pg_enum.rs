//! Every `domain` enum (`PlotStatus`, `PaymentMode`, ...) is stored as
//! plain `text` in Postgres (see database/migrations/0001_init.sql) and
//! already carries `#[serde(rename_all = "snake_case")]` — the exact
//! encoding those columns use. Round-tripping through `serde_json::Value::String`
//! reuses that mapping instead of hand-writing a match arm per enum
//! variant per direction, and keeps `domain` free of a `sqlx` dependency
//! (it's also compiled to wasm for the frontend, where sqlx doesn't run).

use serde::{de::DeserializeOwned, Serialize};

use crate::error::AppError;

pub fn from_pg<T: DeserializeOwned>(column: &str, raw: &str) -> Result<T, AppError> {
    serde_json::from_value(serde_json::Value::String(raw.to_string())).map_err(|_| {
        AppError::Internal(anyhow::anyhow!(
            "unexpected value {raw:?} in column {column}"
        ))
    })
}

pub fn to_pg<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value).expect("domain enums always serialize") {
        serde_json::Value::String(s) => s,
        other => unreachable!("expected a serde_json string for a domain enum, got {other:?}"),
    }
}
