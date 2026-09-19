//! Settings → Users & Access → Roles & Permissions. Manages the
//! `roles` table that already existed (`database/migrations/
//! 0001_init.sql`) but nothing let an admin edit — every organization
//! ran on the single auto-provisioned, everything-permitted "Admin"
//! role from signup until this page. See `crates/backend/src/routes/
//! roles.rs` and `AuthUser::has_permission`
//! (`crates/backend/src/extractors.rs`) for the enforcement side.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState};
use domain::{CreateRoleInput, Role, UpdateRoleInput};

#[component]
pub fn Roles() -> impl IntoView {
    let api = use_api();
    let refresh = RwSignal::new(0u32);
    let show_new = RwSignal::new(false);

    let roles = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move { api.list_roles().await }
        }
    });
    let permissions = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.list_permissions().await }
        }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Roles & permissions"</h1>
                <p>"What each role in your organization is allowed to do."</p>
            </div>
            <button
                type="button"
                class="btn btn-secondary"
                on:click=move |_| show_new.update(|v| *v = !*v)
            >
                {move || if show_new.get() { "Cancel" } else { "+ New role" }}
            </button>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading permissions…" /> }>
            {move || {
                permissions
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(perms) => {
                            let perms_for_form = perms.clone();
                            let perms_for_list = perms;
                            view! {
                                <Show when=move || show_new.get()>
                                    <div class="card form-card" style="margin-bottom: var(--space-4);">
                                        <RoleForm
                                            perms=perms_for_form.clone()
                                            existing=None
                                            on_saved=move || {
                                                refresh.update(|n| *n += 1);
                                                show_new.set(false);
                                            }
                                        />
                                    </div>
                                </Show>

                                <Suspense fallback=|| view! { <LoadingState label="Loading roles…" /> }>
                                    {move || {
                                        let perms = perms_for_list.clone();
                                        roles
                                            .get()
                                            .map(|wrapped| wrapped.take())
                                            .map(|result| match result {
                                                Ok(list) => view! {
                                                    <div class="card-grid">
                                                        {list.into_iter().map(|role| {
                                                            view! {
                                                                <RoleCard
                                                                    role=role
                                                                    perms=perms.clone()
                                                                    on_changed=move || refresh.update(|n| *n += 1)
                                                                />
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                }.into_any(),
                                                Err(e) => view! { <ErrorAlert message=format!("Couldn't load roles: {e}") /> }.into_any(),
                                            })
                                    }}
                                </Suspense>
                            }.into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load permissions: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn RoleCard(
    role: Role,
    perms: Vec<(String, String)>,
    on_changed: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let editing = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let role_id = role.id;
    let assigned_count = role.assigned_user_count;
    let permission_labels: Vec<String> = perms
        .iter()
        .filter(|(perm, _)| role.permissions.contains(perm) || role.permissions.iter().any(|p| p == "*"))
        .map(|(_, label)| label.clone())
        .collect();
    let is_wildcard = role.permissions.iter().any(|p| p == "*");

    let on_delete = {
        let api = api.clone();
        let on_changed = on_changed.clone();
        move |_| {
            if assigned_count > 0 {
                error.set(Some(format!(
                    "This role is still assigned to {assigned_count} user(s) — reassign them first."
                )));
                return;
            }
            error.set(None);
            let api = api.clone();
            let on_changed = on_changed.clone();
            spawn_local(async move {
                match api.delete_role(role_id).await {
                    Ok(()) => on_changed(),
                    Err(e) => error.set(Some(format!("{e}"))),
                }
            });
        }
    };

    view! {
        <div class="card" class:card-grid-item-editing=move || editing.get()>
            <div class="page-header" style="margin-bottom: var(--space-2)">
                <h3 class="mt-0">{role.name.clone()}</h3>
                <span class="meta">{format!("{assigned_count} user(s)")}</span>
            </div>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
            <p class="meta">
                {if is_wildcard {
                    "All permissions".to_string()
                } else if permission_labels.is_empty() {
                    "No permissions granted".to_string()
                } else {
                    permission_labels.join(", ")
                }}
            </p>
            <div style="display:flex; gap: var(--space-2);">
                <button
                    type="button"
                    class="btn btn-secondary"
                    on:click=move |_| editing.update(|v| *v = !*v)
                >
                    {move || if editing.get() { "Cancel" } else { "Edit" }}
                </button>
                <button type="button" class="btn btn-danger" on:click=on_delete>
                    "Delete"
                </button>
            </div>
            <Show when=move || editing.get()>
                <div style="margin-top: var(--space-4); padding-top: var(--space-4); border-top: 1px solid var(--color-border);">
                    <RoleForm
                        perms=perms.clone()
                        existing=Some(role.clone())
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
fn RoleForm(
    perms: Vec<(String, String)>,
    existing: Option<Role>,
    on_saved: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let is_edit = existing.is_some();
    let role_id = existing.as_ref().map(|r| r.id);
    let name = RwSignal::new(existing.as_ref().map(|r| r.name.clone()).unwrap_or_default());
    let selected: RwSignal<Vec<String>> = RwSignal::new(
        existing.as_ref().map(|r| r.permissions.clone()).unwrap_or_default(),
    );
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let toggle_perm = move |perm: String, checked: bool| {
        selected.update(|list| {
            if checked {
                if !list.contains(&perm) {
                    list.push(perm);
                }
            } else {
                list.retain(|p| p != &perm);
            }
        });
    };

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);
        let trimmed = name.get().trim().to_string();
        if trimmed.is_empty() {
            error.set(Some("Enter a role name.".to_string()));
            return;
        }
        submitting.set(true);
        let api = api.clone();
        let on_saved = on_saved.clone();
        let permissions = selected.get();
        spawn_local(async move {
            let result = match role_id {
                Some(id) => api.update_role(id, UpdateRoleInput { name: trimmed, permissions }).await.map(|_| ()),
                None => api.create_role(CreateRoleInput { name: trimmed, permissions }).await.map(|_| ()),
            };
            match result {
                Ok(()) => on_saved(),
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form on:submit=on_submit>
            <h3 class="mt-0">{if is_edit { "Edit role" } else { "New role" }}</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="role-name">"Role name"</label>
                <input
                    id="role-name"
                    type="text"
                    required
                    placeholder="e.g. Sales Manager"
                    prop:value=name
                    on:input=move |ev| name.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label>"Permissions"</label>
                {perms.into_iter().map(|(perm, label)| {
                    let perm_for_check = perm.clone();
                    let perm_for_toggle = perm.clone();
                    let toggle_perm = toggle_perm.clone();
                    view! {
                        <label class="checkbox-field">
                            <input
                                type="checkbox"
                                prop:checked=move || selected.get().contains(&perm_for_check)
                                on:change=move |ev| toggle_perm(perm_for_toggle.clone(), event_target_checked(&ev))
                            />
                            {label}
                        </label>
                    }
                }).collect_view()}
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Saving…" } else { "Save role" }}
            </button>
        </form>
    }
}
