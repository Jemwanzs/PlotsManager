use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::wasm_bindgen::JsCast;
use leptos_router::components::A;

use crate::api::{lead_stage_meta, CreateCustomerInput, CustomerSummary, LeadStage};
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::csv_import::{self, ParsedRow};

#[derive(Clone, Copy, PartialEq)]
enum PipelineFilter {
    All,
    Leads,
    Converted,
    Lost,
}

impl PipelineFilter {
    fn matches(self, summary: &CustomerSummary) -> bool {
        match self {
            PipelineFilter::All => true,
            PipelineFilter::Converted => summary.plots_owned > 0,
            PipelineFilter::Lost => {
                summary.plots_owned == 0 && summary.customer.stage == LeadStage::Lost
            }
            PipelineFilter::Leads => {
                summary.plots_owned == 0 && summary.customer.stage != LeadStage::Lost
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            PipelineFilter::All => "All",
            PipelineFilter::Leads => "Leads",
            PipelineFilter::Converted => "Converted",
            PipelineFilter::Lost => "Lost",
        }
    }
}

#[component]
pub fn CustomersList() -> impl IntoView {
    let api = use_api();
    let search = RwSignal::new(String::new());
    let filter = RwSignal::new(PipelineFilter::All);
    let show_bulk_import = RwSignal::new(false);

    let customers = LocalResource::new(move || {
        let api = api.clone();
        async move { api.list_customers().await }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Customers"</h1>
                <p>"Everyone who has expressed interest in, reserved, or bought a plot."</p>
            </div>
            <div style="display:flex; gap: var(--space-2);">
                <button
                    class="btn btn-secondary"
                    on:click=move |_| show_bulk_import.update(|v| *v = !*v)
                >
                    {move || if show_bulk_import.get() { "Cancel" } else { "Bulk import" }}
                </button>
                <A href="/customers/new" attr:class="btn btn-primary">"+ New customer"</A>
            </div>
        </div>

        <Show when=move || show_bulk_import.get()>
            <BulkCustomerImport on_imported=move || customers.refetch() />
        </Show>

        <input
            class="search-input"
            type="search"
            placeholder="Search by name, phone, or email…"
            prop:value=search
            on:input=move |ev| search.set(event_target_value(&ev))
        />

        <div class="filter-tabs">
            {[
                PipelineFilter::All,
                PipelineFilter::Leads,
                PipelineFilter::Converted,
                PipelineFilter::Lost,
            ]
                .into_iter()
                .map(|f| {
                    view! {
                        <button
                            type="button"
                            class="filter-tab"
                            class:active=move || filter.get() == f
                            on:click=move |_| filter.set(f)
                        >
                            {f.label()}
                        </button>
                    }
                })
                .collect_view()}
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading customers…" /> }>
            {move || {
                customers
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) => {
                            let query = search.get().to_lowercase();
                            let active_filter = filter.get();
                            let filtered: Vec<CustomerSummary> = list
                                .into_iter()
                                .filter(|c| active_filter.matches(c))
                                .filter(|c| {
                                    query.is_empty()
                                        || c.customer.full_name.to_lowercase().contains(&query)
                                        || c.customer.phone.as_deref().unwrap_or("").contains(&query)
                                        || c.customer.email.as_deref().unwrap_or("").to_lowercase().contains(&query)
                                })
                                .collect();

                            if filtered.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{1F464}"
                                        title="Nothing here"
                                        detail="Try a different filter, or a different name, phone number, or email."
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="card-grid">
                                        {filtered
                                            .into_iter()
                                            .map(|c| view! { <CustomerCard summary=c /> })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Err(e) => {
                            view! { <ErrorAlert message=format!("Couldn't load customers: {e}") /> }.into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

/// Uploads a CSV during tenant onboarding — parses client-side
/// (`crate::csv_import::parse_customers_csv`) so the user sees
/// per-row problems before anything reaches the server, then posts
/// only the rows that parsed cleanly to `POST /customers/bulk`, which
/// re-validates each one independently
/// (`crates/backend/src/routes/customers.rs::insert_customer`) — a
/// row that parses fine can still fail there (an ID/passport number
/// already on file). Mirrors `pages/project_detail.rs`'s
/// `BulkPlotImport` — see its docs for the row-number remapping and
/// the double-clone-per-invocation pattern this repeats.
#[component]
fn BulkCustomerImport(on_imported: impl Fn() + Clone + Send + 'static) -> impl IntoView {
    let api = use_api();
    let rows: RwSignal<Vec<ParsedRow<CreateCustomerInput>>> = RwSignal::new(Vec::new());
    let parsing = RwSignal::new(false);
    let importing = RwSignal::new(false);
    let import_result: RwSignal<Option<domain::BulkImportResult>> = RwSignal::new(None);
    let error = RwSignal::new(None::<String>);

    let on_file_change = move |ev: leptos::ev::Event| {
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(files) = input.files() else { return };
        let Some(file) = files.item(0) else { return };
        error.set(None);
        import_result.set(None);
        parsing.set(true);
        spawn_local(async move {
            let text = wasm_bindgen_futures::JsFuture::from(file.text())
                .await
                .ok()
                .and_then(|v| v.as_string());
            match text {
                Some(text) => rows.set(csv_import::parse_customers_csv(&text)),
                None => error.set(Some("Couldn't read that file.".to_string())),
            }
            parsing.set(false);
        });
    };

    let template_href = format!(
        "data:text/csv;charset=utf-8,{}",
        js_sys::encode_uri_component(csv_import::CUSTOMERS_TEMPLATE)
    );

    view! {
        <div class="card" style="margin-bottom: var(--space-4)">
            <h3 class="mt-0">"Bulk import customers"</h3>
            <p class="meta mt-0">
                "CSV columns, in order: full_name, email, phone, id_number, source. Only full_name is required."
            </p>
            <a
                href=template_href
                download="customers_template.csv"
                class="btn btn-secondary"
                style="margin-bottom: var(--space-3); display:inline-block;"
            >
                "Download CSV template"
            </a>

            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="bulk-customers-file">
                    {move || if parsing.get() { "Reading…" } else { "CSV file" }}
                </label>
                <input
                    id="bulk-customers-file"
                    type="file"
                    accept=".csv,text/csv"
                    disabled=parsing
                    on:change=on_file_change
                />
            </div>

            {move || {
                let parsed = rows.get();
                if parsed.is_empty() || import_result.get().is_some() {
                    return None;
                }
                let api = api.clone();
                let on_imported = on_imported.clone();
                let valid_count = parsed.iter().filter(|r| r.result.is_ok()).count();
                let error_count = parsed.len() - valid_count;
                Some(view! {
                    <p>
                        {format!(
                            "{} row(s) found — {} valid, {} with problems.",
                            parsed.len(), valid_count, error_count,
                        )}
                    </p>
                    {(error_count > 0).then(|| view! {
                        <ul class="meta">
                            {parsed
                                .iter()
                                .filter_map(|r| r.result.as_ref().err().map(|e| {
                                    view! { <li>"Row " {r.row} ": " {e.clone()}</li> }
                                }))
                                .collect_view()}
                        </ul>
                    })}
                    <button
                        class="btn btn-primary"
                        disabled=move || importing.get() || valid_count == 0
                        on:click=move |_| {
                            if importing.get() {
                                return;
                            }
                            error.set(None);
                            importing.set(true);
                            let api = api.clone();
                            let on_imported = on_imported.clone();
                            let valid: Vec<(u32, CreateCustomerInput)> = rows
                                .get()
                                .into_iter()
                                .filter_map(|r| r.result.ok().map(|input| (r.row, input)))
                                .collect();
                            let original_rows: Vec<u32> = valid.iter().map(|(row, _)| *row).collect();
                            let inputs: Vec<CreateCustomerInput> =
                                valid.into_iter().map(|(_, input)| input).collect();
                            spawn_local(async move {
                                match api.bulk_create_customers(inputs).await {
                                    Ok(mut r) => {
                                        for e in r.errors.iter_mut() {
                                            if let Some(&orig) = original_rows.get(e.row as usize - 1) {
                                                e.row = orig;
                                            }
                                        }
                                        let had_created = r.created > 0;
                                        import_result.set(Some(r));
                                        if had_created {
                                            on_imported();
                                        }
                                    }
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                                importing.set(false);
                            });
                        }
                    >
                        {move || {
                            if importing.get() {
                                "Importing…".to_string()
                            } else {
                                format!("Import {valid_count} customer(s)")
                            }
                        }}
                    </button>
                })
            }}

            {move || import_result.get().map(|r| view! {
                <div class="alert alert-warning">
                    <p>{format!("{} customer(s) imported.", r.created)}</p>
                    {(!r.errors.is_empty()).then(|| view! {
                        <ul>
                            {r.errors
                                .iter()
                                .map(|e| view! { <li>"Row " {e.row} ": " {e.message.clone()}</li> })
                                .collect_view()}
                        </ul>
                    })}
                </div>
            })}
        </div>
    }
}

#[component]
fn CustomerCard(summary: CustomerSummary) -> impl IntoView {
    let href = format!("/customers/{}", summary.customer.id);
    let converted = summary.plots_owned > 0;
    let (stage_label, stage_color) = if converted {
        ("Converted", "#15734f")
    } else {
        lead_stage_meta(summary.customer.stage)
    };

    view! {
        <A href=href attr:class="project-card card">
            <div class="page-header" style="margin-bottom: var(--space-2)">
                <h3 class="mt-0">{summary.customer.full_name.clone()}</h3>
                <StatusBadge label=stage_label.to_string() color=stage_color.to_string() />
            </div>
            <div class="meta">
                {summary.customer.phone.clone().unwrap_or_else(|| "No phone on file".to_string())}
                " · "
                {summary.customer.email.clone().unwrap_or_else(|| "No email on file".to_string())}
            </div>
            <p class="mt-0">
                {if summary.plots_owned == 0 {
                    "No plots yet".to_string()
                } else if summary.plots_owned == 1 {
                    "1 plot".to_string()
                } else {
                    format!("{} plots", summary.plots_owned)
                }}
            </p>
        </A>
    }
}
