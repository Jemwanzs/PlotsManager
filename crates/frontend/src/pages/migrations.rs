//! The legacy-data migration framework's entry point — see
//! `domain::migration`'s module docs. Upload a CSV here (staged, not
//! written to production yet); `pages/migration_batch_detail.rs` is
//! where a batch's rows get reviewed, exceptions resolved, and
//! committed.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::wasm_bindgen::JsCast;
use leptos_router::components::A;

use crate::auth::{has_permission, use_api, use_auth};
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::csv_import;
use domain::PERM_MIGRATIONS_MANAGE;

#[component]
pub fn Migrations() -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let can_manage = has_permission(auth, PERM_MIGRATIONS_MANAGE);

    let refresh = RwSignal::new(0u32);
    let parsing = RwSignal::new(false);
    let uploading = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let source_system = RwSignal::new("Legacy Excel export".to_string());

    let batches = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move { api.list_migration_batches().await }
        }
    });

    let api_for_upload = api.clone();
    let on_file_change = move |ev: leptos::ev::Event| {
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(files) = input.files() else { return };
        let Some(file) = files.item(0) else { return };
        if parsing.get() || uploading.get() {
            return;
        }
        error.set(None);
        let file_name = file.name();
        let source_system_value = source_system.get();
        let api = api_for_upload.clone();
        parsing.set(true);
        spawn_local(async move {
            let text = wasm_bindgen_futures::JsFuture::from(file.text())
                .await
                .ok()
                .and_then(|v| v.as_string());
            parsing.set(false);
            let Some(text) = text else {
                error.set(Some("Couldn't read that file.".to_string()));
                return;
            };
            let parsed = match csv_import::parse_migration_csv(&text) {
                Ok(p) => p,
                Err(e) => {
                    error.set(Some(e));
                    return;
                }
            };
            uploading.set(true);
            let input = domain::CreateMigrationBatchInput {
                entity_type: domain::MigrationEntityType::Customer,
                source_system: source_system_value,
                source_file_name: file_name,
                rows: parsed.rows,
            };
            match api.create_migration_batch(input).await {
                Ok(_) => refresh.update(|n| *n += 1),
                Err(e) => error.set(Some(format!("{e}"))),
            }
            uploading.set(false);
        });
    };

    let template_href = format!(
        "data:text/csv;charset=utf-8,{}",
        js_sys::encode_uri_component(csv_import::CUSTOMER_MIGRATION_TEMPLATE)
    );

    view! {
        <div class="page-header">
            <div>
                <h1>"Legacy data migration"</h1>
                <p>
                    "Upload a legacy export as its own batch — nothing is written to live "
                    "customer records until you review it and commit. Every row keeps its "
                    "original values on file even if it has a problem, so you can fix and "
                    "retry without re-uploading."
                </p>
            </div>
        </div>

        {can_manage.then(|| view! {
            <div class="card" style="margin-bottom: var(--space-4)">
                <h2 class="mt-0">"Upload a batch"</h2>
                <p class="meta mt-0">
                    "Customer migration only, for now. The file needs a header row — column "
                    "names matter (see the template). legacy_customer_number and full_name are "
                    "required on every row; everything else is optional."
                </p>
                <a
                    href=template_href
                    download="customer_migration_template.csv"
                    class="btn btn-secondary"
                    style="margin-bottom: var(--space-3); display:inline-block;"
                >
                    "Download CSV template"
                </a>

                {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                <div class="form-grid-2">
                    <div class="field">
                        <label for="mig-source-system">"Source system"</label>
                        <input
                            id="mig-source-system"
                            type="text"
                            prop:value=source_system
                            on:input=move |ev| source_system.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="field">
                        <label for="mig-file">
                            {move || if parsing.get() {
                                "Reading…"
                            } else if uploading.get() {
                                "Staging…"
                            } else {
                                "CSV file"
                            }}
                        </label>
                        <input
                            id="mig-file"
                            type="file"
                            accept=".csv,text/csv"
                            disabled=move || parsing.get() || uploading.get()
                            on:change=on_file_change
                        />
                    </div>
                </div>
            </div>
        })}

        <h2>"Batches"</h2>
        <Suspense fallback=|| view! { <LoadingState label="Loading migration batches…" /> }>
            {move || {
                batches.get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) if list.is_empty() => view! {
                            <EmptyState
                                icon="\u{1F4E5}"
                                title="No migration batches yet"
                                detail="Upload a legacy CSV export above to get started."
                            />
                        }.into_any(),
                        Ok(list) => view! {
                            <table class="data-table">
                                <thead>
                                    <tr>
                                        <th>"File"</th>
                                        <th>"Source system"</th>
                                        <th>"Status"</th>
                                        <th>"Rows"</th>
                                        <th>"Uploaded"</th>
                                        <th></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {list.into_iter().map(|b| {
                                        let (status_label, status_color) = match b.status {
                                            domain::MigrationBatchStatus::Staged => ("Staged", "#b45309"),
                                            domain::MigrationBatchStatus::Committed => ("Committed", "#15734f"),
                                        };
                                        let detail_href = format!("/migrations/{}", b.id);
                                        view! {
                                            <tr>
                                                <td>{b.source_file_name.clone()}</td>
                                                <td>{b.source_system.clone()}</td>
                                                <td><StatusBadge label=status_label.to_string() color=status_color.to_string() /></td>
                                                <td>
                                                    {format!("{} total · {} valid · {} exceptions · {} committed",
                                                        b.total_rows, b.valid_rows, b.exception_rows, b.committed_rows)}
                                                </td>
                                                <td><span class="meta">{b.created_at.format("%d %b %Y").to_string()} " · " {b.created_by_name.clone()}</span></td>
                                                <td><A href=detail_href attr:class="btn btn-secondary btn-sm">"Review"</A></td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        }.into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load migration batches: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}
