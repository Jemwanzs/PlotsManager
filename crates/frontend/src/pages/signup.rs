use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;

use crate::api::{ApiError, SignupInput};
use crate::auth::{use_api, use_auth};
use crate::components::ErrorAlert;

#[component]
pub fn Signup() -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let navigate = use_navigate();

    let organization_name = RwSignal::new(String::new());
    let organization_code = RwSignal::new(String::new());
    let admin_full_name = RwSignal::new(String::new());
    let admin_email = RwSignal::new(String::new());
    let admin_password = RwSignal::new(String::new());
    let confirm_password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);

        if admin_password.get() != confirm_password.get() {
            error.set(Some("Those passwords don't match.".to_string()));
            return;
        }

        submitting.set(true);

        let api = api.clone();
        let navigate = navigate.clone();
        let input = SignupInput {
            organization_name: organization_name.get(),
            organization_code: organization_code.get(),
            admin_full_name: admin_full_name.get(),
            admin_email: admin_email.get(),
            admin_password: admin_password.get(),
        };

        spawn_local(async move {
            match api.signup(input).await {
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
                <h1>"Create your organization"</h1>
                <p>"Start a 48-hour trial — no card required."</p>

                {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                <form on:submit=on_submit>
                    <div class="field">
                        <label for="organization_name">"Organization name"</label>
                        <input
                            id="organization_name"
                            type="text"
                            required
                            prop:value=organization_name
                            on:input=move |ev| organization_name.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="field">
                        <label for="organization_code">"Organization code"</label>
                        <input
                            id="organization_code"
                            type="text"
                            required
                            placeholder="e.g. ACACIA"
                            prop:value=organization_code
                            on:input=move |ev| organization_code.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="field">
                        <label for="admin_full_name">"Your name"</label>
                        <input
                            id="admin_full_name"
                            type="text"
                            required
                            prop:value=admin_full_name
                            on:input=move |ev| admin_full_name.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="field">
                        <label for="admin_email">"Email"</label>
                        <input
                            id="admin_email"
                            type="email"
                            autocomplete="username"
                            required
                            prop:value=admin_email
                            on:input=move |ev| admin_email.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="field">
                        <label for="admin_password">"Password"</label>
                        <input
                            id="admin_password"
                            type="password"
                            autocomplete="new-password"
                            required
                            minlength="8"
                            prop:value=admin_password
                            on:input=move |ev| admin_password.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="field">
                        <label for="confirm_password">"Confirm password"</label>
                        <input
                            id="confirm_password"
                            type="password"
                            autocomplete="new-password"
                            required
                            minlength="8"
                            prop:value=confirm_password
                            on:input=move |ev| confirm_password.set(event_target_value(&ev))
                        />
                    </div>
                    <button type="submit" class="btn btn-primary btn-block" disabled=submitting>
                        {move || if submitting.get() { "Creating your account…" } else { "Create organization" }}
                    </button>
                </form>

                <p class="auth-hint">
                    "Already have an account? " <A href="/login">"Sign in"</A>
                </p>
            </div>
        </div>
    }
}
