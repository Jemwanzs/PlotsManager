use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::api::ApiError;
use crate::auth::{use_api, use_auth};
use crate::components::ErrorAlert;

#[component]
pub fn Login() -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let navigate = use_navigate();

    let email = RwSignal::new("admin@acaciagrove.example".to_string());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);
        submitting.set(true);

        let api = api.clone();
        let navigate = navigate.clone();
        let email_value = email.get();
        let password_value = password.get();

        spawn_local(async move {
            match api.login(&email_value, &password_value).await {
                Ok(session) => {
                    auth.set(Some(session));
                    navigate("/", Default::default());
                }
                Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
                Err(e) => error.set(Some(format!("Something went wrong: {e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <div class="auth-screen">
            <div class="auth-card card">
                <div class="auth-brand">
                    <span class="logo-mark">"R"</span>
                    <span>"Real Estate Manager"</span>
                </div>
                <h1>"Sign in"</h1>
                <p>"Access your organisation's projects, plots, and sales."</p>

                {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                <form on:submit=on_submit>
                    <div class="field">
                        <label for="email">"Email"</label>
                        <input
                            id="email"
                            type="email"
                            autocomplete="username"
                            required
                            prop:value=email
                            on:input=move |ev| email.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="field">
                        <label for="password">"Password"</label>
                        <input
                            id="password"
                            type="password"
                            autocomplete="current-password"
                            required
                            prop:value=password
                            on:input=move |ev| password.set(event_target_value(&ev))
                        />
                    </div>
                    <button type="submit" class="btn btn-primary btn-block" disabled=submitting>
                        {move || if submitting.get() { "Signing in…" } else { "Sign in" }}
                    </button>
                </form>

                <p class="auth-hint">
                    "Demo credentials: admin@acaciagrove.example / password123 — this build runs against sample data, not a live backend yet."
                </p>
            </div>
        </div>
    }
}
