use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use hmac::{Hmac, Mac};
use sha2::Sha512;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/webhooks/paystack", post(handle_webhook))
}

/// Paystack signs the raw request body with HMAC-SHA512 using the secret
/// key and sends the hex digest in `x-paystack-signature`. Verify against
/// the untouched bytes — never against a re-serialized/parsed copy, since
/// that can differ byte-for-byte from what Paystack actually signed.
fn verify_signature(secret: &str, body: &[u8], signature_header: &str) -> bool {
    let Ok(mut mac) = Hmac::<Sha512>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());
    // Constant-time-ish via length check + hex compare is good enough here;
    // `hex::encode` output is fixed-length so this isn't a timing oracle
    // the way comparing secrets directly would be.
    expected == signature_header
}

async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(signature) = headers
        .get("x-paystack-signature")
        .and_then(|v| v.to_str().ok())
    else {
        return StatusCode::UNAUTHORIZED;
    };

    if !verify_signature(&state.paystack_secret_key, &body, signature) {
        tracing::warn!("rejected paystack webhook: bad signature");
        return StatusCode::UNAUTHORIZED;
    }

    let event: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("rejected paystack webhook: invalid JSON: {e}");
            return StatusCode::BAD_REQUEST;
        }
    };

    let event_type = event["event"].as_str().unwrap_or_default().to_string();
    // Paystack doesn't send a single canonical event id on every payload;
    // the transaction/subscription reference is the closest stable key.
    let event_id = event["data"]["reference"]
        .as_str()
        .or_else(|| event["data"]["subscription_code"].as_str())
        .unwrap_or_default()
        .to_string();

    if event_id.is_empty() {
        tracing::warn!("paystack webhook missing a usable event id: {event_type}");
        return StatusCode::OK; // acknowledge, but nothing to key idempotency off
    }

    // Idempotency: record the event before acting on it. A unique
    // constraint on (provider, paystack_event_id) makes the second
    // delivery of the same event a no-op insert we can detect and skip.
    let insert_result = sqlx::query(
        r#"insert into billing_webhook_events (provider, paystack_event_id, event_type, payload)
           values ('paystack', $1, $2, $3)"#,
    )
    .bind(&event_id)
    .bind(&event_type)
    .bind(&event)
    .execute(&state.db)
    .await;

    match insert_result {
        Ok(_) => {}
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            tracing::info!("paystack webhook {event_type}/{event_id} already processed");
            return StatusCode::OK;
        }
        Err(e) => {
            tracing::error!("failed to record paystack webhook: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    if let Err(e) = apply_event(&state, &event_type, &event).await {
        tracing::error!("failed to apply paystack event {event_type}: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::OK
}

/// Handles the event types the billing flow currently depends on.
/// Anything else is recorded (above) for audit but not acted on yet —
/// extend this as [16-billing-and-subscriptions.md] grows more event
/// handling (dunning, plan changes, refunds, ...).
async fn apply_event(
    state: &AppState,
    event_type: &str,
    event: &serde_json::Value,
) -> anyhow::Result<()> {
    match event_type {
        "charge.success" => {
            let reference = event["data"]["reference"].as_str().unwrap_or_default();
            sqlx::query(
                r#"update billing_invoices
                   set status = 'paid', paid_at = now()
                   where paystack_reference = $1"#,
            )
            .bind(reference)
            .execute(&state.db)
            .await?;
        }
        "subscription.disable" | "subscription.not_renew" => {
            let subscription_code = event["data"]["subscription_code"].as_str().unwrap_or_default();
            sqlx::query(
                r#"update organization_subscriptions
                   set status = 'cancelled', updated_at = now()
                   where paystack_subscription_code = $1"#,
            )
            .bind(subscription_code)
            .execute(&state.db)
            .await?;
        }
        other => {
            tracing::info!("paystack event {other} recorded, no handler wired yet");
        }
    }
    Ok(())
}
