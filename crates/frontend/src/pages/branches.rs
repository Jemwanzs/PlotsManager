//! Settings -> Branches. A tenant can operate multiple branches
//! (Platform -> Tenant -> Branches -> Users); this page is where a
//! Tenant Admin adds/edits/activates/deactivates them. Assigning users
//! to a branch happens on Settings -> Users & Access instead — that's
//! the many-to-many side of this relationship.

use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState};
use domain::{Branch, CreateBranchInput, TenantUser, UpdateBranchInput};

#[component]
pub fn Branches() -> impl IntoView {
    let api = use_api();
    let refresh = RwSignal::new(0u32);
    let show_new = RwSignal::new(false);

    let page_data = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move {
                let branches = api.list_branches().await?;
                let users = api.list_users().await?;
                Ok::<_, crate::api::ApiError>((branches, users))
            }
        }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Branches"</h1>
                <p>"The locations your organization operates from."</p>
            </div>
            <button
                type="button"
                class="btn btn-secondary"
                on:click=move |_| show_new.update(|v| *v = !*v)
            >
                {move || if show_new.get() { "Cancel" } else { "+ New branch" }}
            </button>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading branches…" /> }>
            {move || {
                page_data
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok((branches, users)) => {
                            let users_for_form = users.clone();
                            view! {
                                <Show when=move || show_new.get()>
                                    <div class="card" style="margin-bottom: var(--space-4); max-width: 480px;">
                                        <BranchForm
                                            managers=users_for_form.clone()
                                            existing=None
                                            on_saved=move || {
                                                refresh.update(|n| *n += 1);
                                                show_new.set(false);
                                            }
                                        />
                                    </div>
                                </Show>

                                {if branches.is_empty() {
                                    view! { <p class="meta">"No branches yet."</p> }.into_any()
                                } else {
                                    view! {
                                        <div class="card-grid">
                                            {branches.into_iter().map(|branch| {
                                                view! {
                                                    <BranchCard
                                                        branch=branch
                                                        managers=users.clone()
                                                        on_changed=move || refresh.update(|n| *n += 1)
                                                    />
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }}
                            }.into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load branches: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn BranchCard(
    branch: Branch,
    managers: Vec<TenantUser>,
    on_changed: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let editing = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let branch_id = branch.id;
    let is_active = branch.is_active;

    let on_toggle_active = {
        let api = api.clone();
        let on_changed = on_changed.clone();
        move |_| {
            error.set(None);
            let api = api.clone();
            let on_changed = on_changed.clone();
            spawn_local(async move {
                let result = if is_active {
                    api.deactivate_branch(branch_id).await
                } else {
                    api.activate_branch(branch_id).await
                };
                match result {
                    Ok(_) => on_changed(),
                    Err(e) => error.set(Some(format!("{e}"))),
                }
            });
        }
    };

    view! {
        <div class="card" class:card-grid-item-editing=move || editing.get()>
            <div class="page-header" style="margin-bottom: var(--space-2)">
                <h3 class="mt-0">{branch.name.clone()} " (" {branch.code.clone()} ")"</h3>
                <span
                    class="badge"
                    style=if branch.is_active {
                        "background-color: var(--color-success)"
                    } else {
                        "background-color: var(--color-text-muted)"
                    }
                >
                    {if branch.is_active { "Active" } else { "Inactive" }}
                </span>
            </div>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
            <p class="meta">{branch.region.clone().unwrap_or_else(|| "No region set".to_string())}</p>
            {branch.location.clone().map(|loc| view! { <p class="meta">{loc}</p> })}
            {branch.contact_name.clone().map(|name| view! {
                <p class="meta">"Contact: " {name} {branch.contact_phone.clone().map(|p| format!(" · {p}"))}</p>
            })}
            {branch.manager_name.clone().map(|name| view! { <p class="meta">"Manager: " {name}</p> })}
            <div style="display:flex; gap: var(--space-2); margin-top: var(--space-3);">
                <button
                    type="button"
                    class="btn btn-secondary"
                    on:click=move |_| editing.update(|v| *v = !*v)
                >
                    {move || if editing.get() { "Cancel" } else { "Edit" }}
                </button>
                <button
                    type="button"
                    class=if is_active { "btn btn-danger" } else { "btn btn-secondary" }
                    on:click=on_toggle_active
                >
                    {if is_active { "Deactivate" } else { "Activate" }}
                </button>
            </div>
            <Show when=move || editing.get()>
                <div style="margin-top: var(--space-4); padding-top: var(--space-4); border-top: 1px solid var(--color-border);">
                    <BranchForm
                        managers=managers.clone()
                        existing=Some(branch.clone())
                        on_saved={
                            let on_changed = on_changed.clone();
                            move || {
                                editing.set(false);
                                on_changed();
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn BranchForm(
    managers: Vec<TenantUser>,
    existing: Option<Branch>,
    on_saved: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let is_edit = existing.is_some();
    let branch_id = existing.as_ref().map(|b| b.id);
    let name = RwSignal::new(existing.as_ref().map(|b| b.name.clone()).unwrap_or_default());
    let code = RwSignal::new(existing.as_ref().map(|b| b.code.clone()).unwrap_or_default());
    let region = RwSignal::new(existing.as_ref().and_then(|b| b.region.clone()).unwrap_or_default());
    let location = RwSignal::new(existing.as_ref().and_then(|b| b.location.clone()).unwrap_or_default());
    let contact_name = RwSignal::new(existing.as_ref().and_then(|b| b.contact_name.clone()).unwrap_or_default());
    let contact_phone = RwSignal::new(existing.as_ref().and_then(|b| b.contact_phone.clone()).unwrap_or_default());
    let initial_manager_id =
        existing.as_ref().and_then(|b| b.manager_id).map(|id| id.to_string()).unwrap_or_default();
    let manager_id = RwSignal::new(initial_manager_id.clone());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);

        let manager_raw = manager_id.get();
        let manager_uuid = if manager_raw.trim().is_empty() {
            None
        } else {
            match Uuid::parse_str(manager_raw.trim()) {
                Ok(v) => Some(v),
                Err(_) => {
                    error.set(Some("Choose a valid manager.".to_string()));
                    return;
                }
            }
        };
        let opt = |s: String| if s.trim().is_empty() { None } else { Some(s) };

        submitting.set(true);
        let api = api.clone();
        let on_saved = on_saved.clone();
        let name_value = name.get();
        let code_value = code.get();
        let region_value = opt(region.get());
        let location_value = opt(location.get());
        let contact_name_value = opt(contact_name.get());
        let contact_phone_value = opt(contact_phone.get());

        if let Some(id) = branch_id {
            spawn_local(async move {
                let result = api
                    .update_branch(
                        id,
                        UpdateBranchInput {
                            name: name_value,
                            code: code_value,
                            region: region_value,
                            location: location_value,
                            contact_name: contact_name_value,
                            contact_phone: contact_phone_value,
                            manager_id: manager_uuid,
                        },
                    )
                    .await;
                match result {
                    Ok(_) => on_saved(),
                    Err(e) => error.set(Some(format!("{e}"))),
                }
                submitting.set(false);
            });
        } else {
            spawn_local(async move {
                let result = api
                    .create_branch(CreateBranchInput {
                        name: name_value,
                        code: code_value,
                        region: region_value,
                        location: location_value,
                        contact_name: contact_name_value,
                        contact_phone: contact_phone_value,
                        manager_id: manager_uuid,
                    })
                    .await;
                match result {
                    Ok(_) => on_saved(),
                    Err(e) => error.set(Some(format!("{e}"))),
                }
                submitting.set(false);
            });
        }
    };

    view! {
        <form on:submit=on_submit>
            <h3 class="mt-0">{if is_edit { "Edit branch" } else { "New branch" }}</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="form-grid-2">
                <div class="field">
                    <label for="branch-name">"Branch name"</label>
                    <input
                        id="branch-name"
                        type="text"
                        required
                        placeholder="e.g. Nairobi Branch"
                        prop:value=name
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </div>
                <div class="field">
                    <label for="branch-code">"Branch code"</label>
                    <input
                        id="branch-code"
                        type="text"
                        required
                        placeholder="e.g. NBO"
                        prop:value=code
                        on:input=move |ev| code.set(event_target_value(&ev))
                    />
                </div>
                <div class="field">
                    <label for="branch-region">"Region (optional)"</label>
                    <input
                        id="branch-region"
                        type="text"
                        prop:value=region
                        on:input=move |ev| region.set(event_target_value(&ev))
                    />
                </div>
                <div class="field">
                    <label for="branch-location">"Location (optional)"</label>
                    <input
                        id="branch-location"
                        type="text"
                        prop:value=location
                        on:input=move |ev| location.set(event_target_value(&ev))
                    />
                </div>
                <div class="field">
                    <label for="branch-contact-name">"Contact name (optional)"</label>
                    <input
                        id="branch-contact-name"
                        type="text"
                        prop:value=contact_name
                        on:input=move |ev| contact_name.set(event_target_value(&ev))
                    />
                </div>
                <div class="field">
                    <label for="branch-contact-phone">"Contact phone (optional)"</label>
                    <input
                        id="branch-contact-phone"
                        type="text"
                        prop:value=contact_phone
                        on:input=move |ev| contact_phone.set(event_target_value(&ev))
                    />
                </div>
            </div>
            <div class="field">
                <label for="branch-manager">"Branch manager (optional)"</label>
                {if managers.is_empty() {
                    view! { <p class="meta">"No users yet to choose as manager."</p> }.into_any()
                } else {
                    view! {
                        <select
                            id="branch-manager"
                            on:change=move |ev| manager_id.set(event_target_value(&ev))
                        >
                            <option value="" selected=initial_manager_id.is_empty()>"No manager"</option>
                            {managers.into_iter().map(|u| {
                                let id = u.id.to_string();
                                let is_selected = id == initial_manager_id;
                                view! { <option value=id selected=is_selected>{u.full_name}</option> }
                            }).collect_view()}
                        </select>
                    }.into_any()
                }}
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Saving…" } else { "Save branch" }}
            </button>
        </form>
    }
}
