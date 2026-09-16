use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::api::{ApiError, CreateProjectInput};
use crate::auth::use_api;
use crate::components::ErrorAlert;
use domain::AreaUnit;

#[component]
pub fn NewProject() -> impl IntoView {
    let api = use_api();
    let navigate = use_navigate();

    let name = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let location = RwSignal::new(String::new());
    let total_size = RwSignal::new(String::new());
    let area_unit = RwSignal::new("acres".to_string());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);
    let generating = RwSignal::new(false);

    let on_generate = {
        let api = api.clone();
        move |_| {
            if generating.get() {
                return;
            }
            generating.set(true);
            let api = api.clone();
            spawn_local(async move {
                match api.next_number("project", None).await {
                    Ok(number) => code.set(number),
                    Err(e) => error.set(Some(format!("Couldn't generate a project code: {e}"))),
                }
                generating.set(false);
            });
        }
    };

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);

        let Ok(size) = Decimal::from_str(total_size.get().trim()) else {
            error.set(Some("Enter a valid total size.".to_string()));
            return;
        };
        let unit = match area_unit.get().as_str() {
            "hectares" => AreaUnit::Hectares,
            "square_metres" => AreaUnit::SquareMetres,
            _ => AreaUnit::Acres,
        };

        submitting.set(true);
        let api = api.clone();
        let navigate = navigate.clone();
        let input = CreateProjectInput {
            name: name.get(),
            code: code.get(),
            location: location.get(),
            total_size: size,
            area_unit: unit,
        };

        spawn_local(async move {
            match api.create_project(input).await {
                Ok(project) => navigate(&format!("/projects/{}", project.id), Default::default()),
                Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
                Err(e) => error.set(Some(format!("Something went wrong: {e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <div class="page-header">
            <div>
                <h1>"New project"</h1>
                <p>"Register a land project, then add its plots."</p>
            </div>
        </div>

        <div class="card" style="max-width: 480px;">
            <form on:submit=on_submit>
                {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                <div class="field">
                    <label for="name">"Project name"</label>
                    <input
                        id="name"
                        type="text"
                        required
                        prop:value=name
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </div>

                <div class="field">
                    <label for="code">"Project code"</label>
                    <div style="display:flex; gap: var(--space-2);">
                        <input
                            id="code"
                            type="text"
                            required
                            placeholder="e.g. RM-P2"
                            prop:value=code
                            on:input=move |ev| code.set(event_target_value(&ev))
                        />
                        <button
                            type="button"
                            class="btn btn-secondary"
                            disabled=generating
                            on:click=on_generate
                        >
                            {move || if generating.get() { "…" } else { "Auto-generate" }}
                        </button>
                    </div>
                </div>

                <div class="field">
                    <label for="location">"Location"</label>
                    <input
                        id="location"
                        type="text"
                        required
                        placeholder="e.g. Malaa, Machakos"
                        prop:value=location
                        on:input=move |ev| location.set(event_target_value(&ev))
                    />
                </div>

                <div class="field">
                    <label for="size">"Total size"</label>
                    <input
                        id="size"
                        type="text"
                        inputmode="decimal"
                        required
                        prop:value=total_size
                        on:input=move |ev| total_size.set(event_target_value(&ev))
                    />
                </div>

                <div class="field">
                    <label for="unit">"Unit"</label>
                    <select id="unit" prop:value=area_unit on:change=move |ev| area_unit.set(event_target_value(&ev))>
                        <option value="acres">"Acres"</option>
                        <option value="hectares">"Hectares"</option>
                        <option value="square_metres">"Square metres"</option>
                    </select>
                </div>

                <button type="submit" class="btn btn-primary" disabled=submitting>
                    {move || if submitting.get() { "Saving…" } else { "Save project" }}
                </button>
            </form>
        </div>
    }
}
