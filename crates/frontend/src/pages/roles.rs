//! Settings → Users & Access → Roles & Permissions. Manages the
//! `roles` table that already existed (`database/migrations/
//! 0001_init.sql`) but nothing let an admin edit — every organization
//! ran on the single auto-provisioned, everything-permitted "Admin"
//! role from signup until this page. See `crates/backend/src/routes/
//! roles.rs` and `AuthUser::has_permission`
//! (`crates/backend/src/extractors.rs`) for the enforcement side, and
//! `crates/domain/src/permissions.rs` for the Module -> Feature ->
//! Action registry this editor renders.

use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashSet;

use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState};
use domain::{CreateRoleInput, PermissionDef, Role, UpdateRoleInput, PERM_WILDCARD};

/// One feature's permissions, in registry order.
#[derive(Clone)]
struct FeatureGroup {
    name: String,
    perms: Vec<PermissionDef>,
}

/// One module's features, in registry order.
struct ModuleGroup {
    name: String,
    features: Vec<FeatureGroup>,
}

/// Groups the flat, already-ordered registry into Module -> Feature ->
/// Action sections. Relies on the registry listing entries for the
/// same (module, feature) pair consecutively (`domain::permissions`'s
/// own doc comment states this as the authoring contract) — a plain
/// sequential group-by, not a sort, so registry display order is
/// preserved exactly.
fn group_permissions(perms: &[PermissionDef]) -> Vec<ModuleGroup> {
    let mut modules: Vec<ModuleGroup> = Vec::new();
    for p in perms {
        let module = match modules.last_mut() {
            Some(m) if m.name == p.module => m,
            _ => {
                modules.push(ModuleGroup { name: p.module.clone(), features: Vec::new() });
                modules.last_mut().unwrap()
            }
        };
        let feature = match module.features.last_mut() {
            Some(f) if f.name == p.feature => f,
            _ => {
                module.features.push(FeatureGroup { name: p.feature.clone(), perms: Vec::new() });
                module.features.last_mut().unwrap()
            }
        };
        feature.perms.push(p.clone());
    }
    modules
}

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
                                    <div class="card form-card" style="margin-bottom: var(--space-4); max-width: none;">
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
    perms: Vec<PermissionDef>,
    on_changed: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let editing = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let role_id = role.id;
    let assigned_count = role.assigned_user_count;
    let permission_labels: Vec<String> = perms
        .iter()
        .filter(|p| role.permissions.contains(&p.key) || role.permissions.iter().any(|p| p == PERM_WILDCARD))
        .map(|p| p.label.clone())
        .collect();
    let is_wildcard = role.permissions.iter().any(|p| p == PERM_WILDCARD);

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
                    "Full access — every current and future permission".to_string()
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
    perms: Vec<PermissionDef>,
    existing: Option<Role>,
    on_saved: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let is_edit = existing.is_some();
    let role_id = existing.as_ref().map(|r| r.id);
    let assigned_count = existing.as_ref().map(|r| r.assigned_user_count).unwrap_or(0);
    let name = RwSignal::new(existing.as_ref().map(|r| r.name.clone()).unwrap_or_default());

    let existing_is_wildcard = existing.as_ref().is_some_and(|r| r.permissions.iter().any(|p| p == PERM_WILDCARD));
    // The set this role actually had *before* this edit, resolved to
    // concrete keys even if it was granted via "*" — this is what
    // "currently relied upon by assigned users" is compared against
    // when warning about removed permissions, not the literal stored
    // ["*"] string.
    let originally_granted: HashSet<String> = if existing_is_wildcard {
        perms.iter().map(|p| p.key.clone()).collect()
    } else {
        existing.as_ref().map(|r| r.permissions.iter().cloned().collect()).unwrap_or_default()
    };

    let full_access = RwSignal::new(existing_is_wildcard);
    let selected: RwSignal<HashSet<String>> = RwSignal::new(
        if existing_is_wildcard {
            HashSet::new()
        } else {
            existing.as_ref().map(|r| r.permissions.iter().cloned().collect()).unwrap_or_default()
        },
    );
    let search = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);
    let confirm_removal = RwSignal::new(false);

    let groups = group_permissions(&perms);
    // One expand/collapse flag per module, paired by index with
    // `groups` — default collapsed so ~10 modules don't turn this into
    // the same long-page problem the flat checkbox list had.
    let expanded: Vec<RwSignal<bool>> = groups.iter().map(|_| RwSignal::new(false)).collect();

    let toggle_perm = move |key: String, checked: bool| {
        selected.update(|set| {
            if checked {
                set.insert(key);
            } else {
                set.remove(&key);
            }
        });
        confirm_removal.set(false);
    };

    let perms_for_submit = perms.clone();
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

        let new_permissions: Vec<String> = if full_access.get() {
            vec![PERM_WILDCARD.to_string()]
        } else {
            selected.get().into_iter().collect()
        };

        // Warn once before actually removing a capability that assigned
        // users currently rely on — the second click (button now reads
        // "Confirm & save") proceeds.
        if is_edit && assigned_count > 0 && !confirm_removal.get() {
            let new_set: HashSet<&str> = if full_access.get() {
                originally_granted.iter().map(|s| s.as_str()).collect()
            } else {
                new_permissions.iter().map(|s| s.as_str()).collect()
            };
            let removed: Vec<&str> = originally_granted
                .iter()
                .map(|s| s.as_str())
                .filter(|k| !new_set.contains(k))
                .collect();
            if !removed.is_empty() {
                let removed_labels: Vec<String> = perms_for_submit
                    .iter()
                    .filter(|p| removed.contains(&p.key.as_str()))
                    .map(|p| p.label.clone())
                    .collect();
                error.set(Some(format!(
                    "This role is assigned to {assigned_count} user(s). Saving will immediately remove: {}. Click \"Confirm & save\" to proceed.",
                    removed_labels.join(", ")
                )));
                confirm_removal.set(true);
                return;
            }
        }

        submitting.set(true);
        let api = api.clone();
        let on_saved = on_saved.clone();
        spawn_local(async move {
            let result = match role_id {
                Some(id) => api.update_role(id, UpdateRoleInput { name: trimmed, permissions: new_permissions }).await.map(|_| ()),
                None => api.create_role(CreateRoleInput { name: trimmed, permissions: new_permissions }).await.map(|_| ()),
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
            {move || error.get().map(|msg| view! {
                <div class=move || if confirm_removal.get() { "perm-removal-warning" } else { "" }>
                    <ErrorAlert message=msg />
                </div>
            })}

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

            <div class="perm-wildcard-banner">
                <label class="checkbox-field" style="margin-bottom: 0;">
                    <input
                        type="checkbox"
                        prop:checked=full_access
                        on:change=move |ev| {
                            let checked = event_target_checked(&ev);
                            full_access.set(checked);
                            if !checked {
                                selected.set(HashSet::new());
                            }
                            confirm_removal.set(false);
                        }
                    />
                    <span>"Full access — every current and future permission"</span>
                </label>
                <span class="meta">"New capabilities added to the platform later grant automatically."</span>
            </div>

            <Show when=move || !full_access.get()>
                <div class="perm-toolbar">
                    <input
                        type="search"
                        class="perm-search"
                        placeholder="Search modules, features or permissions…"
                        prop:value=search
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                    <button
                        type="button"
                        class="btn btn-secondary"
                        on:click={
                            let perms = perms.clone();
                            move |_| {
                                selected.set(perms.iter().map(|p| p.key.clone()).collect());
                                confirm_removal.set(false);
                            }
                        }
                    >
                        "Select all"
                    </button>
                    <button
                        type="button"
                        class="btn btn-secondary"
                        on:click=move |_| {
                            selected.set(HashSet::new());
                            confirm_removal.set(false);
                        }
                    >
                        "Clear all"
                    </button>
                </div>

                {
                    groups.iter().zip(expanded.iter().copied()).map(|(module, is_expanded)| {
                        let module_perms: Vec<String> = module.features.iter().flat_map(|f| f.perms.iter().map(|p| p.key.clone())).collect();
                        let module_perms_for_all = module_perms.clone();
                        let module_perms_for_clear = module_perms.clone();
                        let module_has_sensitive = module.features.iter().any(|f| f.perms.iter().any(|p| p.sensitive));

                        let module_name_for_match = module.name.clone();
                        let module_features_for_match: Vec<(String, Vec<String>)> = module.features.iter()
                            .map(|f| (f.name.clone(), f.perms.iter().map(|p| p.label.clone()).collect()))
                            .collect();

                        // Whether this module should render at all given
                        // the current search text — empty search always
                        // matches.
                        let module_matches = move || {
                            let q = search.get().to_lowercase();
                            if q.is_empty() { return true; }
                            module_name_for_match.to_lowercase().contains(&q)
                                || module_features_for_match.iter().any(|(fname, labels)| {
                                    fname.to_lowercase().contains(&q) || labels.iter().any(|l| l.to_lowercase().contains(&q))
                                })
                        };
                        let module_matches_for_style = module_matches.clone();
                        // Open state: while searching, force every
                        // matching module open (so results are visible
                        // without a manual click); with no search text,
                        // fall back to the manually-toggled state.
                        let effective_open = move || {
                            if search.get().trim().is_empty() { is_expanded.get() } else { module_matches() }
                        };

                        let module_name_for_header = module.name.clone();
                        // Owned so the `<Show>` body below (called fresh
                        // every time the module is expanded/collapsed or
                        // a search happens) can rebuild this module's
                        // feature rows without borrowing from `groups`,
                        // which doesn't outlive this outer `.map()` call.
                        let module_features_owned = module.features.clone();
                        let render_features = move || {
                            module_features_owned.iter().map(|f| {
                                let f_perms = f.perms.clone();
                                let f_perms_for_all = f_perms.clone();
                                let f_perms_for_clear = f_perms.clone();
                                let f_name = f.name.clone();
                                let f_name_for_match = f.name.clone();
                                let f_perms_for_match = f_perms.clone();

                                view! {
                                    <div
                                        class="perm-feature"
                                        style=move || {
                                            let q = search.get().to_lowercase();
                                            if q.is_empty() { return String::new(); }
                                            let matches = f_name_for_match.to_lowercase().contains(&q)
                                                || f_perms_for_match.iter().any(|p| p.label.to_lowercase().contains(&q));
                                            if matches { String::new() } else { "display:none;".to_string() }
                                        }
                                    >
                                        <div class="perm-feature-header">
                                            <span class="perm-feature-name">{f_name.clone()}</span>
                                            <div class="perm-feature-actions">
                                                <button
                                                    type="button"
                                                    on:click=move |_| {
                                                        selected.update(|set| { for p in &f_perms_for_all { set.insert(p.key.clone()); } });
                                                        confirm_removal.set(false);
                                                    }
                                                >
                                                    "Select all"
                                                </button>
                                                <button
                                                    type="button"
                                                    on:click=move |_| {
                                                        selected.update(|set| { for p in &f_perms_for_clear { set.remove(&p.key); } });
                                                        confirm_removal.set(false);
                                                    }
                                                >
                                                    "Clear"
                                                </button>
                                            </div>
                                        </div>
                                        <div class="perm-grid">
                                            {f_perms.clone().into_iter().map(|p| {
                                                let key_a = p.key.clone();
                                                let key_b = p.key.clone();
                                                let key_c = p.key.clone();
                                                view! {
                                                    <label class="perm-chip" class:selected=move || selected.get().contains(&key_a)>
                                                        <input
                                                            type="checkbox"
                                                            prop:checked=move || selected.get().contains(&key_b)
                                                            on:change=move |ev| toggle_perm(key_c.clone(), event_target_checked(&ev))
                                                        />
                                                        {if p.sensitive { view! { <span class="perm-chip-sensitive-dot" title="Sensitive"></span> }.into_any() } else { ().into_any() }}
                                                        {p.label.clone()}
                                                    </label>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }
                            }).collect_view()
                        };

                        view! {
                            <div class="perm-module" style=move || if module_matches_for_style() { String::new() } else { "display:none;".to_string() }>
                                <button
                                    type="button"
                                    class="perm-module-header"
                                    on:click=move |_| is_expanded.update(|v| *v = !*v)
                                >
                                    <span class="perm-module-chevron" class:open=effective_open.clone()>"▸"</span>
                                    <span class="perm-module-name">{module_name_for_header.clone()}</span>
                                    <span class="perm-module-count">
                                        {move || {
                                            let sel = selected.get();
                                            let count = module_perms.iter().filter(|k| sel.contains(*k)).count();
                                            format!("{count}/{} selected", module_perms.len())
                                        }}
                                    </span>
                                    {if module_has_sensitive {
                                        view! { <span class="perm-module-sensitive-badge">"Sensitive"</span> }.into_any()
                                    } else { ().into_any() }}
                                    <span class="perm-module-actions">
                                        <button
                                            type="button"
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                selected.update(|set| { for k in &module_perms_for_all { set.insert(k.clone()); } });
                                                confirm_removal.set(false);
                                            }
                                        >
                                            "Select all"
                                        </button>
                                        <button
                                            type="button"
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                selected.update(|set| { for k in &module_perms_for_clear { set.remove(k); } });
                                                confirm_removal.set(false);
                                            }
                                        >
                                            "Clear"
                                        </button>
                                    </span>
                                </button>
                                <Show when=effective_open>
                                    <div class="perm-module-body">{render_features.clone()}</div>
                                </Show>
                            </div>
                        }
                    }).collect_view()
                }
            </Show>

            <div style="margin-top: var(--space-4);">
                <button type="submit" class="btn btn-primary" disabled=submitting>
                    {move || {
                        if submitting.get() {
                            "Saving…"
                        } else if confirm_removal.get() {
                            "Confirm & save"
                        } else if is_edit {
                            "Save changes"
                        } else {
                            "Create role"
                        }
                    }}
                </button>
            </div>
        </form>
    }
}
