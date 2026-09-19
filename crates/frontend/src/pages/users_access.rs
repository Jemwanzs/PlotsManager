//! Settings -> Users & Access. Tenant Admins manage the users belonging
//! to their own organization: add, edit (name/email/mobile/branch/
//! role), and activate/deactivate. See `crates/backend/src/routes/
//! users.rs` for the enforcement side (`PERM_MANAGE_USERS`, same
//! catalog entry `Roles & permissions` already exposes). Password
//! reset and session revocation are a later phase of the same spec —
//! this page only sets a temporary password at creation time.

use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState, PasswordField};
use domain::{Branch, CreateUserInput, Role, TenantUser, UpdateUserInput};

#[component]
pub fn UsersAccess() -> impl IntoView {
    let api = use_api();
    let refresh = RwSignal::new(0u32);
    let show_new = RwSignal::new(false);

    let page_data = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move {
                let users = api.list_users().await?;
                let roles = api.list_roles().await?;
                let branches = api.list_branches().await?;
                Ok::<_, crate::api::ApiError>((users, roles, branches))
            }
        }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Users & access"</h1>
                <p>"Who can sign in to your organization, and what they're assigned."</p>
            </div>
            <button
                type="button"
                class="btn btn-secondary"
                on:click=move |_| show_new.update(|v| *v = !*v)
            >
                {move || if show_new.get() { "Cancel" } else { "+ New user" }}
            </button>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading users…" /> }>
            {move || {
                page_data
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok((users, roles, branches)) => {
                            let roles_for_form = roles.clone();
                            let branches_for_form = branches.clone();
                            view! {
                                <Show when=move || show_new.get()>
                                    <div class="card" style="margin-bottom: var(--space-4); max-width: 560px;">
                                        <UserForm
                                            roles=roles_for_form.clone()
                                            branches=branches_for_form.clone()
                                            existing=None
                                            on_saved=move || {
                                                refresh.update(|n| *n += 1);
                                                show_new.set(false);
                                            }
                                        />
                                    </div>
                                </Show>

                                {if users.is_empty() {
                                    view! { <p class="meta">"No users yet."</p> }.into_any()
                                } else {
                                    view! {
                                        <div class="card">
                                            <div class="table-scroll">
                                                <table class="data-table">
                                                    <thead>
                                                        <tr>
                                                            <th>"Name"</th>
                                                            <th>"Email"</th>
                                                            <th>"Mobile"</th>
                                                            <th>"Role"</th>
                                                            <th>"Branch"</th>
                                                            <th>"Status"</th>
                                                            <th>"Last login"</th>
                                                            <th></th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {users.into_iter().map(|user| {
                                                            view! {
                                                                <UserRow
                                                                    user=user
                                                                    roles=roles.clone()
                                                                    branches=branches.clone()
                                                                    on_changed=move || refresh.update(|n| *n += 1)
                                                                />
                                                            }
                                                        }).collect_view()}
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    }.into_any()
                                }}
                            }.into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load users: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn UserRow(
    user: TenantUser,
    roles: Vec<Role>,
    branches: Vec<Branch>,
    on_changed: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let editing = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let resetting = RwSignal::new(false);
    let revoking = RwSignal::new(false);
    let user_id = user.id;
    let is_active = user.is_active;

    let on_toggle_active = {
        let api = api.clone();
        let on_changed = on_changed.clone();
        move |_| {
            error.set(None);
            let api = api.clone();
            let on_changed = on_changed.clone();
            spawn_local(async move {
                let result = if is_active {
                    api.deactivate_user(user_id).await
                } else {
                    api.activate_user(user_id).await
                };
                match result {
                    Ok(_) => on_changed(),
                    Err(e) => error.set(Some(format!("{e}"))),
                }
            });
        }
    };

    let on_revoke_sessions = {
        let api = api.clone();
        let on_changed = on_changed.clone();
        move |_| {
            error.set(None);
            revoking.set(true);
            let api = api.clone();
            let on_changed = on_changed.clone();
            spawn_local(async move {
                match api.revoke_user_sessions(user_id).await {
                    Ok(_) => on_changed(),
                    Err(e) => error.set(Some(format!("{e}"))),
                }
                revoking.set(false);
            });
        }
    };

    // Two separate clones so each `<Show>` block below's own
    // auto-generated `move` closure captures its own copy — a shared
    // `on_changed` would get moved into the first block whole, even
    // though only a `.clone()` of it is used inside.
    let on_changed_for_edit = on_changed.clone();
    let on_changed_for_reset = on_changed.clone();

    view! {
        <tr>
            <td>{user.full_name.clone()}</td>
            <td>{user.email.clone()}</td>
            <td>{user.mobile.clone().unwrap_or_else(|| "—".to_string())}</td>
            <td>{user.role_name.clone().unwrap_or_else(|| "No role".to_string())}</td>
            <td>{user.branch_name.clone().unwrap_or_else(|| "—".to_string())}</td>
            <td>
                <span
                    class="badge"
                    style=if user.is_active {
                        "background-color: var(--color-success)"
                    } else {
                        "background-color: var(--color-text-muted)"
                    }
                >
                    {if user.is_active { "Active" } else { "Inactive" }}
                </span>
            </td>
            <td>
                {user.last_login_at.map(|t| t.format("%b %d, %Y %H:%M").to_string()).unwrap_or_else(|| "Never".to_string())}
            </td>
            <td style="display:flex; gap: var(--space-2); flex-wrap: wrap;">
                <button type="button" class="btn btn-secondary" on:click=move |_| editing.update(|v| *v = !*v)>
                    {move || if editing.get() { "Cancel" } else { "Edit" }}
                </button>
                <button
                    type="button"
                    class=if is_active { "btn btn-danger" } else { "btn btn-secondary" }
                    on:click=on_toggle_active
                >
                    {if is_active { "Deactivate" } else { "Activate" }}
                </button>
                <button
                    type="button"
                    class="btn btn-secondary"
                    on:click=move |_| resetting.update(|v| *v = !*v)
                >
                    {move || if resetting.get() { "Cancel" } else { "Reset password" }}
                </button>
                <button type="button" class="btn btn-secondary" disabled=revoking on:click=on_revoke_sessions>
                    {move || if revoking.get() { "Revoking…" } else { "Revoke sessions" }}
                </button>
            </td>
        </tr>
        {move || error.get().map(|msg| view! {
            <tr><td colspan="8"><ErrorAlert message=msg /></td></tr>
        })}
        <Show when=move || editing.get()>
            <tr>
                <td colspan="8">
                    <UserForm
                        roles=roles.clone()
                        branches=branches.clone()
                        existing=Some(user.clone())
                        on_saved={
                            let on_changed = on_changed_for_edit.clone();
                            move || {
                                editing.set(false);
                                on_changed();
                            }
                        }
                    />
                </td>
            </tr>
        </Show>
        <Show when=move || resetting.get()>
            <tr>
                <td colspan="8">
                    <ResetPasswordForm
                        user_id=user_id
                        on_saved={
                            let on_changed = on_changed_for_reset.clone();
                            move || {
                                resetting.set(false);
                                on_changed();
                            }
                        }
                    />
                </td>
            </tr>
        </Show>
    }
}

#[component]
fn ResetPasswordForm(user_id: Uuid, on_saved: impl Fn() + Clone + Send + Sync + 'static) -> impl IntoView {
    let api = use_api();
    let temporary_password = RwSignal::new(String::new());
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
        let on_saved = on_saved.clone();
        let input = domain::ResetPasswordInput { temporary_password: temporary_password.get() };
        spawn_local(async move {
            match api.reset_user_password(user_id, input).await {
                Ok(_) => on_saved(),
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form on:submit=on_submit>
            <h3 class="mt-0">"Set a new temporary password"</h3>
            <p class="meta">"They'll be required to change it the next time they sign in."</p>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
            <PasswordField
                id="reset-temp-password"
                label="Temporary password"
                value=temporary_password
                autocomplete="new-password"
                minlength=8
                show_strength=true
            />
            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Saving…" } else { "Set temporary password" }}
            </button>
        </form>
    }
}

#[component]
fn UserForm(
    roles: Vec<Role>,
    branches: Vec<Branch>,
    existing: Option<TenantUser>,
    on_saved: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let is_edit = existing.is_some();
    let user_id = existing.as_ref().map(|u| u.id);
    let full_name = RwSignal::new(existing.as_ref().map(|u| u.full_name.clone()).unwrap_or_default());
    let email = RwSignal::new(existing.as_ref().map(|u| u.email.clone()).unwrap_or_default());
    let mobile = RwSignal::new(existing.as_ref().and_then(|u| u.mobile.clone()).unwrap_or_default());
    let initial_branch_id =
        existing.as_ref().and_then(|u| u.branch_id).map(|id| id.to_string()).unwrap_or_default();
    let initial_role_id =
        existing.as_ref().and_then(|u| u.role_id).map(|id| id.to_string()).unwrap_or_default();
    let branch_id = RwSignal::new(initial_branch_id.clone());
    let role_id = RwSignal::new(initial_role_id.clone());
    let temporary_password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);

        let Ok(role_uuid) = Uuid::parse_str(role_id.get().trim()) else {
            error.set(Some("Choose a role.".to_string()));
            return;
        };
        let branch_raw = branch_id.get();
        let branch_uuid = if branch_raw.trim().is_empty() {
            None
        } else {
            match Uuid::parse_str(branch_raw.trim()) {
                Ok(v) => Some(v),
                Err(_) => {
                    error.set(Some("Choose a valid branch.".to_string()));
                    return;
                }
            }
        };
        let mobile_raw = mobile.get();
        let mobile_value = if mobile_raw.trim().is_empty() { None } else { Some(mobile_raw) };
        let full_name_value = full_name.get();
        let email_value = email.get();

        submitting.set(true);
        let api = api.clone();
        let on_saved = on_saved.clone();

        if let Some(id) = user_id {
            spawn_local(async move {
                let result = api
                    .update_user(
                        id,
                        UpdateUserInput {
                            full_name: full_name_value,
                            email: email_value,
                            mobile: mobile_value,
                            branch_id: branch_uuid,
                            role_id: role_uuid,
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
            let temp_pw = temporary_password.get();
            spawn_local(async move {
                let result = api
                    .create_user(CreateUserInput {
                        full_name: full_name_value,
                        email: email_value,
                        mobile: mobile_value,
                        branch_id: branch_uuid,
                        role_id: role_uuid,
                        temporary_password: temp_pw,
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
            <h3 class="mt-0">{if is_edit { "Edit user" } else { "New user" }}</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="user-name">"Name"</label>
                <input
                    id="user-name"
                    type="text"
                    required
                    prop:value=full_name
                    on:input=move |ev| full_name.set(event_target_value(&ev))
                />
            </div>
            <div class="field">
                <label for="user-email">"Email"</label>
                <input
                    id="user-email"
                    type="email"
                    autocomplete="username"
                    required
                    prop:value=email
                    on:input=move |ev| email.set(event_target_value(&ev))
                />
            </div>
            <div class="field">
                <label for="user-mobile">"Mobile (optional)"</label>
                <input
                    id="user-mobile"
                    type="text"
                    prop:value=mobile
                    on:input=move |ev| mobile.set(event_target_value(&ev))
                />
            </div>
            <div class="field">
                <label for="user-role">"Role"</label>
                <select
                    id="user-role"
                    required
                    on:change=move |ev| role_id.set(event_target_value(&ev))
                >
                    <option value="" selected=initial_role_id.is_empty()>"Select a role…"</option>
                    {roles.into_iter().map(|r| {
                        let id = r.id.to_string();
                        let is_selected = id == initial_role_id;
                        view! { <option value=id selected=is_selected>{r.name}</option> }
                    }).collect_view()}
                </select>
            </div>
            <div class="field">
                <label for="user-branch">"Branch (optional)"</label>
                {if branches.is_empty() {
                    view! { <p class="meta">"No branches yet — add one under Settings → Branches."</p> }.into_any()
                } else {
                    view! {
                        <select
                            id="user-branch"
                            on:change=move |ev| branch_id.set(event_target_value(&ev))
                        >
                            <option value="" selected=initial_branch_id.is_empty()>"No branch"</option>
                            {branches.into_iter().map(|b| {
                                let id = b.id.to_string();
                                let is_selected = id == initial_branch_id;
                                view! { <option value=id selected=is_selected>{b.name}</option> }
                            }).collect_view()}
                        </select>
                    }.into_any()
                }}
            </div>
            {if is_edit {
                view! {}.into_any()
            } else {
                view! {
                    <PasswordField
                        id="user-temp-password"
                        label="Temporary password"
                        value=temporary_password
                        autocomplete="new-password"
                        minlength=8
                        show_strength=true
                    />
                }.into_any()
            }}

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || {
                    if submitting.get() {
                        "Saving…"
                    } else if is_edit {
                        "Save changes"
                    } else {
                        "Create user"
                    }
                }}
            </button>
        </form>
    }
}
