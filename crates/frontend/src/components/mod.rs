use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::wasm_bindgen::JsCast;
use uuid::Uuid;

use crate::auth::{has_permission, use_api, use_auth};
use crate::icons::{Icon, IconName};
use domain::{DocumentEntityType, PERM_DOCUMENTS_MANAGE};

mod charts;
pub use charts::{BarChart, ChartPoint, DonutChart, DonutSegment, LineChart};

#[component]
pub fn StatCard(
    label: &'static str,
    value: String,
    #[prop(optional)] sub: Option<String>,
) -> impl IntoView {
    view! {
        <div class="stat-card">
            <div class="stat-label">{label}</div>
            <div class="stat-value">{value}</div>
            {sub.map(|s| view! { <div class="stat-sub">{s}</div> })}
        </div>
    }
}

#[component]
pub fn StatusBadge(label: String, color: String) -> impl IntoView {
    view! {
        <span class="badge" style=format!("background-color: {color}")>
            {label}
        </span>
    }
}

#[component]
pub fn LoadingState(#[prop(optional)] label: Option<&'static str>) -> impl IntoView {
    view! {
        <div class="loading-state">
            <span class="spinner"></span>
            <span>{label.unwrap_or("Loading…")}</span>
        </div>
    }
}

#[component]
pub fn EmptyState(icon: &'static str, title: &'static str, detail: &'static str) -> impl IntoView {
    view! {
        <div class="empty-state">
            <div class="icon">{icon}</div>
            <h3>{title}</h3>
            <p>{detail}</p>
        </div>
    }
}

#[component]
pub fn ErrorAlert(message: String) -> impl IntoView {
    view! { <div class="alert alert-danger">{message}</div> }
}

/// A password `<input>` with a standard show/hide eye toggle, reused by
/// every password field across the app (login, signup, and the
/// create-user/reset-password/change-password forms that reuse this same
/// component rather than re-implementing the toggle each time).
#[component]
pub fn PasswordField(
    id: &'static str,
    label: &'static str,
    value: RwSignal<String>,
    #[prop(default = "current-password")] autocomplete: &'static str,
    #[prop(optional)] minlength: Option<u32>,
    #[prop(default = false)] show_strength: bool,
) -> impl IntoView {
    let visible = RwSignal::new(false);
    let minlength_attr = minlength.map(|m| m.to_string());

    view! {
        <div class="field">
            <label for=id>{label}</label>
            <div class="password-input-wrap">
                <input
                    id=id
                    type=move || if visible.get() { "text" } else { "password" }
                    autocomplete=autocomplete
                    required
                    minlength=minlength_attr
                    prop:value=value
                    on:input=move |ev| value.set(event_target_value(&ev))
                />
                <button
                    type="button"
                    class="password-toggle"
                    aria-label=move || if visible.get() { "Hide password" } else { "Show password" }
                    on:click=move |_| visible.update(|v| *v = !*v)
                >
                    {move || if visible.get() {
                        view! { <Icon name=IconName::EyeOff /> }.into_any()
                    } else {
                        view! { <Icon name=IconName::Eye /> }.into_any()
                    }}
                </button>
            </div>
            {show_strength.then(|| view! { <PasswordStrengthMeter value=value /> })}
        </div>
    }
}

/// 0 (empty) to 4 (strong) — length plus character-class variety, no
/// external crate needed for this level of feedback.
pub fn password_strength(pw: &str) -> (u8, &'static str) {
    if pw.is_empty() {
        return (0, "");
    }
    let mut score: u8 = 0;
    if pw.len() >= 8 {
        score += 1;
    }
    if pw.len() >= 12 {
        score += 1;
    }
    let has_lower = pw.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = pw.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = pw.chars().any(|c| c.is_ascii_digit());
    let has_symbol = pw.chars().any(|c| !c.is_ascii_alphanumeric());
    let variety = [has_lower, has_upper, has_digit, has_symbol]
        .iter()
        .filter(|b| **b)
        .count();
    if variety >= 3 {
        score += 1;
    }
    if variety == 4 && pw.len() >= 10 {
        score += 1;
    }
    score = score.min(4);
    let label = match score {
        0 => "Very weak",
        1 => "Weak",
        2 => "Fair",
        3 => "Good",
        _ => "Strong",
    };
    (score, label)
}

fn format_file_size(bytes: i64) -> String {
    let bytes = bytes as f64;
    if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 {
        format!("{:.0} KB", bytes / 1024.0)
    } else {
        format!("{bytes:.0} B")
    }
}

const DOCUMENT_TYPES: &[&str] = &[
    "Passport Photo",
    "National ID (Front)",
    "National ID (Back)",
    "Passport",
    "KRA PIN Certificate",
    "Purchase Agreement",
    "Booking Form",
    "Payment Evidence",
    "Title Deed",
    "Survey Plan",
    "Transfer Document",
    "Consent Document",
    "Correspondence",
    "Other",
];

/// A reusable document vault panel: attaches to any entity that has a
/// `documents` row (`database/migrations/0027_documents.sql`) via
/// `(entity_type, entity_id)` — a customer, a plot, a project, a sale,
/// a loan account, or a payment. Dropped into any detail page with
/// just those two props; no per-entity wiring beyond that.
#[component]
pub fn DocumentsPanel(entity_type: DocumentEntityType, entity_id: Uuid) -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let can_manage = has_permission(auth, PERM_DOCUMENTS_MANAGE);

    let refresh = RwSignal::new(0u32);
    let upload_open = RwSignal::new(false);
    let upload_error = RwSignal::new(None::<String>);
    let uploading = RwSignal::new(false);
    let delete_error = RwSignal::new(None::<String>);

    let doc_type = RwSignal::new(DOCUMENT_TYPES[0].to_string());
    let doc_number = RwSignal::new(String::new());
    let issue_date = RwSignal::new(String::new());
    let expiry_date = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());

    let docs = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move { api.list_documents(entity_type, entity_id).await }
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
        if uploading.get() {
            return;
        }
        upload_error.set(None);
        uploading.set(true);
        let api = api_for_upload.clone();
        let input = domain::UploadDocumentInput {
            entity_type,
            entity_id,
            document_type: doc_type.get(),
            document_number: Some(doc_number.get()).filter(|s| !s.trim().is_empty()),
            issue_date: chrono::NaiveDate::parse_from_str(&issue_date.get(), "%Y-%m-%d").ok(),
            expiry_date: chrono::NaiveDate::parse_from_str(&expiry_date.get(), "%Y-%m-%d").ok(),
            description: Some(description.get()).filter(|s| !s.trim().is_empty()),
        };
        spawn_local(async move {
            match api.upload_document(input, file).await {
                Ok(_) => {
                    doc_number.set(String::new());
                    issue_date.set(String::new());
                    expiry_date.set(String::new());
                    description.set(String::new());
                    upload_open.set(false);
                    refresh.update(|n| *n += 1);
                }
                Err(e) => upload_error.set(Some(format!("{e}"))),
            }
            uploading.set(false);
        });
    };

    let api_for_delete = api.clone();

    view! {
        <div class="card form-card" style="margin-bottom: var(--space-5)">
            <div class="page-header" style="margin-bottom: var(--space-3)">
                <h2 class="mt-0">"Documents"</h2>
                {can_manage.then(|| {
                    view! {
                        <button
                            type="button"
                            class="btn btn-secondary"
                            on:click=move |_| upload_open.update(|v| *v = !*v)
                        >
                            {move || if upload_open.get() { "Cancel" } else { "Upload document" }}
                        </button>
                    }
                })}
            </div>

            {move || delete_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            {move || if upload_open.get() {
                let on_file_change = on_file_change.clone();
                view! {
                    <div class="form-grid-2" style="margin-bottom: var(--space-3)">
                        <div class="field">
                            <label for="doc-type">"Document type"</label>
                            <select id="doc-type" prop:value=doc_type on:change=move |ev| doc_type.set(event_target_value(&ev))>
                                {DOCUMENT_TYPES.iter().map(|t| view! { <option value=*t>{*t}</option> }).collect_view()}
                            </select>
                        </div>
                        <div class="field">
                            <label for="doc-number">"Document number"</label>
                            <input id="doc-number" type="text" prop:value=doc_number on:input=move |ev| doc_number.set(event_target_value(&ev)) />
                        </div>
                        <div class="field">
                            <label for="doc-issue-date">"Issue date"</label>
                            <input id="doc-issue-date" type="date" prop:value=issue_date on:input=move |ev| issue_date.set(event_target_value(&ev)) />
                        </div>
                        <div class="field">
                            <label for="doc-expiry-date">"Expiry date"</label>
                            <input id="doc-expiry-date" type="date" prop:value=expiry_date on:input=move |ev| expiry_date.set(event_target_value(&ev)) />
                        </div>
                        <div class="field" style="grid-column: 1 / -1">
                            <label for="doc-description">"Description"</label>
                            <input id="doc-description" type="text" prop:value=description on:input=move |ev| description.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    {move || upload_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
                    <div class="field" style="max-width: 360px;">
                        <label for="doc-file">
                            {move || if uploading.get() { "Uploading…" } else { "File (PDF, JPEG, or PNG, up to 10MB)" }}
                        </label>
                        <input id="doc-file" type="file" accept="application/pdf,image/jpeg,image/png" disabled=uploading on:change=on_file_change />
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            <Suspense fallback=|| view! { <LoadingState label="Loading documents…" /> }>
                {move || {
                    docs.get()
                        .map(|wrapped| wrapped.take())
                        .map(|result| match result {
                            Ok(list) if list.is_empty() => view! {
                                <p class="meta">"No documents attached yet."</p>
                            }.into_any(),
                            Ok(list) => {
                                let api_for_url = api.clone();
                                let api_for_delete = api_for_delete.clone();
                                view! {
                                    <table class="data-table">
                                        <thead>
                                            <tr>
                                                <th>"Type"</th>
                                                <th>"File"</th>
                                                <th>"Number"</th>
                                                <th>"Uploaded"</th>
                                                <th>"Size"</th>
                                                <th></th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {list.into_iter().map(|doc| {
                                                let file_url = api_for_url.document_file_url(doc.id);
                                                let doc_id = doc.id;
                                                let api_for_delete = api_for_delete.clone();
                                                view! {
                                                    <tr>
                                                        <td>{doc.document_type.clone()}</td>
                                                        <td>
                                                            <a href=file_url target="_blank" rel="noopener noreferrer">
                                                                {doc.original_filename.clone()}
                                                            </a>
                                                        </td>
                                                        <td>{doc.document_number.clone().unwrap_or_else(|| "—".to_string())}</td>
                                                        <td>
                                                            <span class="meta">
                                                                {doc.uploaded_at.format("%d %b %Y").to_string()} " · " {doc.uploaded_by_name.clone()}
                                                            </span>
                                                        </td>
                                                        <td>{format_file_size(doc.file_size)}</td>
                                                        <td>
                                                            {can_manage.then(|| {
                                                                let api_for_delete = api_for_delete.clone();
                                                                view! {
                                                                    <button
                                                                        type="button"
                                                                        class="btn btn-danger btn-sm"
                                                                        on:click=move |_| {
                                                                            let api = api_for_delete.clone();
                                                                            delete_error.set(None);
                                                                            spawn_local(async move {
                                                                                match api.delete_document(doc_id).await {
                                                                                    Ok(_) => refresh.update(|n| *n += 1),
                                                                                    Err(e) => delete_error.set(Some(format!("Couldn't delete: {e}"))),
                                                                                }
                                                                            });
                                                                        }
                                                                    >
                                                                        "Delete"
                                                                    </button>
                                                                }
                                                            })}
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                }.into_any()
                            }
                            Err(e) => view! { <ErrorAlert message=format!("Couldn't load documents: {e}") /> }.into_any(),
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn PasswordStrengthMeter(value: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="password-strength">
            <div class="password-strength-bars">
                {(1..=4u8).map(|i| {
                    view! {
                        <div class=move || {
                            let (score, _) = password_strength(&value.get());
                            if i > score {
                                "password-strength-bar".to_string()
                            } else {
                                let tier = match score {
                                    0 | 1 => "weak",
                                    2 => "fair",
                                    3 => "good",
                                    _ => "strong",
                                };
                                format!("password-strength-bar filled-{tier}")
                            }
                        }></div>
                    }
                }).collect_view()}
            </div>
            <span class="password-strength-label">{move || password_strength(&value.get()).1}</span>
        </div>
    }
}
