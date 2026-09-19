//! Profile -> Security -> Change Password. Also the mandatory gate an
//! admin-created or admin-reset account lands on: `AppShell`
//! (`crates/frontend/src/layout/mod.rs`) redirects here whenever
//! `auth.get().user.must_change_password` is true, and this same page
//! serves both cases — there's nothing about the form itself that
//! differs, only whether leaving without submitting is allowed.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::api::ApiError;
use crate::auth::{use_api, use_auth};
use crate::components::{ErrorAlert, PasswordField};
use domain::ChangePasswordInput;

#[component]
pub fn ChangePassword() -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let navigate = use_navigate();

    let forced = move || auth.get().map(|s| s.user.must_change_password).unwrap_or(false);

    let current_password = RwSignal::new(String::new());
    let new_password = RwSignal::new(String::new());
    let confirm_password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let success = RwSignal::new(false);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);
        success.set(false);

        if new_password.get() != confirm_password.get() {
            error.set(Some("Those passwords don't match.".to_string()));
            return;
        }

        submitting.set(true);
        let api = api.clone();
        let auth = auth;
        let navigate = navigate.clone();
        let input = ChangePasswordInput {
            current_password: current_password.get(),
            new_password: new_password.get(),
        };

        spawn_local(async move {
            match api.change_password(input).await {
                Ok(session) => {
                    auth.set(Some(session));
                    success.set(true);
                    current_password.set(String::new());
                    new_password.set(String::new());
                    confirm_password.set(String::new());
                    navigate("/", Default::default());
                }
                Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
                Err(e) => error.set(Some(format!("Something went wrong: {e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <div class="page-header">
            <div>
                <h1>"Change your password"</h1>
                {move || if forced() {
                    view! {
                        <p>"A temporary password was set for your account — choose a new one before continuing."</p>
                    }.into_any()
                } else {
                    view! { <p>"Profile → Security."</p> }.into_any()
                }}
            </div>
        </div>

        <div class="card form-card">
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
            {move || {
                if success.get() {
                    view! { <div class="alert alert-success">"Password changed."</div> }.into_any()
                } else {
                    view! {}.into_any()
                }
            }}

            <form on:submit=on_submit>
                <PasswordField
                    id="current-password"
                    label="Current password"
                    value=current_password
                    autocomplete="current-password"
                />
                <PasswordField
                    id="new-password"
                    label="New password"
                    value=new_password
                    autocomplete="new-password"
                    minlength=8
                    show_strength=true
                />
                <PasswordField
                    id="confirm-new-password"
                    label="Confirm new password"
                    value=confirm_password
                    autocomplete="new-password"
                    minlength=8
                />

                <button type="submit" class="btn btn-primary" disabled=submitting>
                    {move || if submitting.get() { "Saving…" } else { "Change password" }}
                </button>
            </form>
        </div>
    }
}
