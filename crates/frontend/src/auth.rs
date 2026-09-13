use leptos::prelude::*;

use crate::api::{ApiClient, AuthSession};

/// Current session, provided at the app root. `None` means signed out.
/// Not persisted (no localStorage) yet — a reload signs you out. That's a
/// deliberate scope cut for this pass, not an oversight; add
/// `gloo-storage` here when it matters.
pub type AuthSignal = RwSignal<Option<AuthSession>>;

pub fn use_auth() -> AuthSignal {
    use_context::<AuthSignal>().expect("AuthSignal not provided — is this inside <App/>?")
}

pub fn use_api() -> ApiClient {
    use_context::<ApiClient>().expect("ApiClient not provided — is this inside <App/>?")
}
