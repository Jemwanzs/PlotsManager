use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::config::Config;

#[derive(Debug, Clone, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub user: AuthUser,
}

#[derive(Debug, Deserialize)]
struct AuthErrorBody {
    #[serde(alias = "error_description", alias = "msg")]
    message: String,
}

async fn parse_auth_response(resp: gloo_net::http::Response) -> Result<Session, String> {
    if resp.ok() {
        resp.json::<Session>()
            .await
            .map_err(|e| format!("failed to parse Supabase Auth response: {e}"))
    } else {
        let status = resp.status();
        let message = resp
            .json::<AuthErrorBody>()
            .await
            .map(|b| b.message)
            .unwrap_or_else(|_| format!("Supabase Auth request failed ({status})"));
        Err(message)
    }
}

/// Registers a new user under an *existing* organization. Creating a new
/// organization (and its first admin user) is a separate, privileged flow
/// — not implemented here yet, see docs/16-billing-and-subscriptions.md
/// for how org sign-up ties into the Paystack subscription flow.
pub async fn sign_up(
    cfg: &Config,
    email: &str,
    password: &str,
    full_name: &str,
    organization_id: Uuid,
) -> Result<Session, String> {
    let body = json!({
        "email": email,
        "password": password,
        "data": {
            "full_name": full_name,
            "organization_id": organization_id,
        },
    });

    let resp = gloo_net::http::Request::post(&cfg.auth_url("signup"))
        .header("apikey", cfg.anon_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    parse_auth_response(resp).await
}

pub async fn sign_in(cfg: &Config, email: &str, password: &str) -> Result<Session, String> {
    let body = json!({ "email": email, "password": password });

    let resp = gloo_net::http::Request::post(&cfg.auth_url("token?grant_type=password"))
        .header("apikey", cfg.anon_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    parse_auth_response(resp).await
}

pub async fn sign_out(cfg: &Config, access_token: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::post(&cfg.auth_url("logout"))
        .header("apikey", cfg.anon_key)
        .header("Authorization", &format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.ok() {
        Ok(())
    } else {
        Err(format!("sign-out failed ({})", resp.status()))
    }
}
