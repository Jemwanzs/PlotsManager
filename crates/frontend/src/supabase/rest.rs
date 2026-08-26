use serde::de::DeserializeOwned;

use super::config::Config;

/// Generic PostgREST `select`. `query` is the raw querystring PostgREST
/// expects, e.g. `"select=*&status=eq.available"` — see
/// <https://postgrest.org/en/stable/references/api/tables_views.html>.
/// RLS on the target table (see `supabase/migrations/`) decides what the
/// caller's `access_token` actually returns; this function does not
/// itself enforce tenant scoping.
pub async fn select<T: DeserializeOwned>(
    cfg: &Config,
    access_token: &str,
    table: &str,
    query: &str,
) -> Result<Vec<T>, String> {
    let url = format!("{}?{query}", cfg.rest_url(table));

    let resp = gloo_net::http::Request::get(&url)
        .header("apikey", cfg.anon_key)
        .header("Authorization", &format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.ok() {
        resp.json::<Vec<T>>()
            .await
            .map_err(|e| format!("failed to parse PostgREST response from {table}: {e}"))
    } else {
        Err(format!(
            "PostgREST GET {table} failed ({}): {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ))
    }
}
