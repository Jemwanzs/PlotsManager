use leptos::prelude::*;

use crate::api::{ApiClient, AuthSession};

/// Current session, provided at the app root. `None` means signed out.
/// Not persisted (no localStorage) yet — a reload signs you out. That's a
/// deliberate scope cut for this pass, not an oversight; add
/// `gloo-storage` here when it matters.
pub type AuthSignal = RwSignal<Option<AuthSession>>;

/// The signed-in organization's configured currency code (e.g. "KES",
/// "USD") — provided at the app root, populated from `GET
/// /api/v1/settings` once `AuthSignal` goes `Some` (see `app.rs`), reset
/// to the "KES" default on sign-out. Every money-formatting call site
/// reads this instead of hardcoding a currency, so a non-KES tenant
/// (Settings → General → Currency) sees their own currency everywhere.
pub type CurrencySignal = RwSignal<String>;

pub fn use_auth() -> AuthSignal {
    use_context::<AuthSignal>().expect("AuthSignal not provided — is this inside <App/>?")
}

pub fn use_currency() -> CurrencySignal {
    use_context::<CurrencySignal>()
        .expect("CurrencySignal not provided — is this inside <App/>?")
}

pub fn use_api() -> ApiClient {
    use_context::<ApiClient>().expect("ApiClient not provided — is this inside <App/>?")
}

/// Mirrors `AuthUser::has_permission` (`crates/backend/src/
/// extractors.rs`) on the frontend — the backend is still the actual
/// enforcement, this is only so a role that can't do something never
/// sees the button for it in the first place. Call inside a reactive
/// closure (`move || has_permission(auth, "...")`) so it re-evaluates
/// if the session changes.
pub fn has_permission(auth: AuthSignal, permission: &str) -> bool {
    auth.get()
        .map(|s| {
            s.user.is_platform_owner
                || s.user.permissions.iter().any(|p| p == "*" || p == permission)
        })
        .unwrap_or(false)
}
