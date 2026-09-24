//! One migration batch's staging rows — review, resolve exceptions
//! (edit a row's fields, re-validated in place), and commit. See
//! `domain::migration`'s module docs for the overall flow and
//! `pages/migrations.rs` for the upload step that creates a batch.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::auth::{has_permission, use_api, use_auth};
use crate::components::{ErrorAlert, LoadingState, StatusBadge};
use domain::PERM_MIGRATIONS_MANAGE;

/// The customer-migration template's columns, in the order the edit
/// form shows them — see `csv_import::CUSTOMER_MIGRATION_TEMPLATE`.
const CUSTOMER_FIELDS: &[(&str, &str)] = &[
    ("legacy_customer_number", "Legacy customer number"),
    ("full_name", "Full name"),
    ("id_number", "National ID / Passport"),
    ("email", "Email"),
    ("phone", "Phone"),
    ("title", "Title"),
    ("customer_type", "Customer type (individual/company/joint)"),
    ("kra_pin", "KRA PIN"),
    ("postal_address", "Postal address"),
    ("city", "City"),
    ("physical_address", "Physical address"),
    ("next_of_kin_name", "Next of kin name"),
    ("next_of_kin_relationship", "Next of kin relationship"),
    ("next_of_kin_mobile", "Next of kin mobile"),
    ("next_of_kin_id_number", "Next of kin ID number"),
    ("next_of_kin_address", "Next of kin address"),
];

fn json_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

#[component]
pub fn MigrationBatchDetailPage() -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let can_manage = has_permission(auth, PERM_MIGRATIONS_MANAGE);
    let params = use_params_map();
    let batch_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let refresh = RwSignal::new(0u32);
    let editing_id = RwSignal::new(None::<Uuid>);
    let edit_fields: RwSignal<HashMap<String, String>> = RwSignal::new(HashMap::new());
    let edit_error = RwSignal::new(None::<String>);
    let edit_saving = RwSignal::new(false);
    let commit_error = RwSignal::new(None::<String>);
    let committing = RwSignal::new(false);
    let commit_result = RwSignal::new(None::<domain::MigrationCommitResult>);

    let batch = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move {
                match batch_id() {
                    Some(id) => Some(api.get_migration_batch(id).await),
                    None => None,
                }
            }
        }
    });

    let rows = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move {
                match batch_id() {
                    Some(id) => Some(api.list_migration_rows(id).await),
                    None => None,
                }
            }
        }
    });

    let api_for_save = api.clone();
    let on_save_edit = move |row_id: Uuid| {
        let Some(bid) = batch_id() else { return };
        if edit_saving.get() {
            return;
        }
        edit_error.set(None);
        edit_saving.set(true);
        let api = api_for_save.clone();
        let fields = edit_fields.get();
        spawn_local(async move {
            match api.update_migration_row(bid, row_id, domain::UpdateMigrationRowInput { fields }).await {
                Ok(_) => {
                    editing_id.set(None);
                    refresh.update(|n| *n += 1);
                }
                Err(e) => edit_error.set(Some(format!("{e}"))),
            }
            edit_saving.set(false);
        });
    };

    let api_for_commit = api.clone();
    let on_commit = move |_| {
        let Some(bid) = batch_id() else { return };
        if committing.get() {
            return;
        }
        commit_error.set(None);
        committing.set(true);
        let api = api_for_commit.clone();
        spawn_local(async move {
            match api.commit_migration_batch(bid).await {
                Ok(r) => {
                    commit_result.set(Some(r));
                    refresh.update(|n| *n += 1);
                }
                Err(e) => commit_error.set(Some(format!("{e}"))),
            }
            committing.set(false);
        });
    };

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading batch…" /> }>
            {move || {
                batch.get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(b) => {
                            let can_commit = can_manage
                                && b.status == domain::MigrationBatchStatus::Staged
                                && b.valid_rows > 0;
                            let (status_label, status_color) = match b.status {
                                domain::MigrationBatchStatus::Staged => ("Staged", "#b45309"),
                                domain::MigrationBatchStatus::Committed => ("Committed", "#15734f"),
                            };
                            view! {
                                <div class="page-header">
                                    <div>
                                        <h1>{b.source_file_name.clone()}</h1>
                                        <p>
                                            {b.source_system.clone()} " · "
                                            {format!("{} total · {} valid · {} exceptions · {} committed",
                                                b.total_rows, b.valid_rows, b.exception_rows, b.committed_rows)}
                                        </p>
                                    </div>
                                    <StatusBadge label=status_label.to_string() color=status_color.to_string() />
                                </div>

                                {move || commit_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
                                {move || commit_result.get().map(|r| view! {
                                    <div class="alert alert-warning">
                                        <p class="mt-0">
                                            {format!(
                                                "{} committed · {} skipped (still exceptions) · {} already committed earlier.",
                                                r.committed, r.skipped_exceptions, r.already_committed,
                                            )}
                                        </p>
                                    </div>
                                })}

                                {can_commit.then(|| {
                                    let on_commit = on_commit.clone();
                                    view! {
                                    <button
                                        type="button"
                                        class="btn btn-primary"
                                        style="margin-bottom: var(--space-4)"
                                        disabled=move || committing.get()
                                        on:click=on_commit
                                    >
                                        {move || if committing.get() {
                                            "Committing…".to_string()
                                        } else {
                                            format!("Commit {} valid row(s)", b.valid_rows)
                                        }}
                                    </button>
                                }})}
                            }.into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this batch: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>

        <h2>"Rows"</h2>
        <Suspense fallback=|| view! { <LoadingState label="Loading rows…" /> }>
            {move || {
                rows.get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(list) => {
                            view! {
                                <table class="data-table">
                                    <thead>
                                        <tr>
                                            <th>"Row"</th>
                                            <th>"Legacy #"</th>
                                            <th>"Full name"</th>
                                            <th>"Status"</th>
                                            <th>"Problem"</th>
                                            <th></th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {list.into_iter().map(|row| {
                                            let row_id = row.id;
                                            let is_editing = move || editing_id.get() == Some(row_id);
                                            let raw = row.raw_data.clone();
                                            let (status_label, status_color) = match row.status {
                                                domain::MigrationRowStatus::Valid => ("Valid", "#15734f"),
                                                domain::MigrationRowStatus::Exception => ("Exception", "#b91c1c"),
                                            };
                                            let already_committed = row.committed_entity_id.is_some();
                                            let full_name = json_str(&row.normalized_data, "full_name");
                                            let legacy_number = json_str(&row.normalized_data, "legacy_customer_number");
                                            let on_save_edit = on_save_edit.clone();
                                            view! {
                                                <tr>
                                                    <td>{row.source_row}</td>
                                                    <td>{legacy_number}</td>
                                                    <td>{full_name}</td>
                                                    <td><StatusBadge label=status_label.to_string() color=status_color.to_string() /></td>
                                                    <td><span class="meta">{row.exception_message.clone().unwrap_or_default()}</span></td>
                                                    <td>
                                                        {(can_manage && !already_committed).then(|| {
                                                            let raw = raw.clone();
                                                            view! {
                                                                <button
                                                                    type="button"
                                                                    class="btn btn-secondary btn-sm"
                                                                    on:click=move |_| {
                                                                        if is_editing() {
                                                                            editing_id.set(None);
                                                                        } else {
                                                                            let mut fields = HashMap::new();
                                                                            for (key, _) in CUSTOMER_FIELDS {
                                                                                fields.insert(key.to_string(), json_str(&raw, key));
                                                                            }
                                                                            edit_fields.set(fields);
                                                                            edit_error.set(None);
                                                                            editing_id.set(Some(row_id));
                                                                        }
                                                                    }
                                                                >
                                                                    {move || if is_editing() { "Cancel" } else { "Edit" }}
                                                                </button>
                                                            }
                                                        })}
                                                        {already_committed.then(|| view! {
                                                            <span class="meta">"Committed"</span>
                                                        })}
                                                    </td>
                                                </tr>
                                                {move || if is_editing() {
                                                    let on_save_edit = on_save_edit.clone();
                                                    view! {
                                                        <tr>
                                                            <td colspan="6">
                                                                <div class="form-grid-2" style="margin: var(--space-2) 0">
                                                                    {CUSTOMER_FIELDS.iter().map(|(key, label)| {
                                                                        let key = key.to_string();
                                                                        let key_for_input = key.clone();
                                                                        let value = Signal::derive(move || {
                                                                            edit_fields.get().get(&key).cloned().unwrap_or_default()
                                                                        });
                                                                        view! {
                                                                            <div class="field">
                                                                                <label>{*label}</label>
                                                                                <input
                                                                                    type="text"
                                                                                    prop:value=value
                                                                                    on:input=move |ev| {
                                                                                        edit_fields.update(|f| {
                                                                                            f.insert(key_for_input.clone(), event_target_value(&ev));
                                                                                        });
                                                                                    }
                                                                                />
                                                                            </div>
                                                                        }
                                                                    }).collect_view()}
                                                                </div>
                                                                {move || edit_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
                                                                <button
                                                                    type="button"
                                                                    class="btn btn-primary btn-sm"
                                                                    disabled=move || edit_saving.get()
                                                                    on:click=move |_| on_save_edit(row_id)
                                                                >
                                                                    {move || if edit_saving.get() { "Saving…" } else { "Save & re-validate" }}
                                                                </button>
                                                            </td>
                                                        </tr>
                                                    }.into_any()
                                                } else {
                                                    view! {}.into_any()
                                                }}
                                            }
                                        }).collect_view()}
                                    </tbody>
                                </table>
                            }.into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load rows: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}
