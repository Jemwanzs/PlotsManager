//! Bulk-imports *historical* sales during tenant onboarding — see
//! `domain::BulkSaleRow`'s module docs for why this is a separate
//! path from a fresh reservation (`pages/project_detail.rs`'s
//! `ReserveForm`/`QuoteForm`): a migrated sale usually isn't at zero
//! paid, so this carries the actual amount already repaid instead of
//! assuming a brand-new Lipa Pole Pole account. Mirrors
//! `pages/project_detail.rs`'s `BulkPlotImport` / `pages/customers_list.rs`'s
//! `BulkCustomerImport` — see their docs for the row-number remapping
//! and the double-clone-per-invocation pattern this repeats.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::wasm_bindgen::JsCast;

use crate::api::BulkSaleRow;
use crate::auth::use_api;
use crate::components::ErrorAlert;
use crate::csv_import::{self, ParsedRow};

#[component]
pub fn BulkSalesImport() -> impl IntoView {
    let api = use_api();
    let rows: RwSignal<Vec<ParsedRow<BulkSaleRow>>> = RwSignal::new(Vec::new());
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
                Some(text) => rows.set(csv_import::parse_sales_csv(&text)),
                None => error.set(Some("Couldn't read that file.".to_string())),
            }
            parsing.set(false);
        });
    };

    let template_href = format!(
        "data:text/csv;charset=utf-8,{}",
        js_sys::encode_uri_component(csv_import::SALES_TEMPLATE)
    );

    view! {
        <div class="page-header">
            <div>
                <h1>"Bulk import sales"</h1>
                <p>
                    "For migrating historical sales during onboarding — plots and customers "
                    "referenced here must already exist. A Lipa Pole Pole row's amount_paid "
                    "carries over the real balance already repaid, not zero."
                </p>
            </div>
        </div>

        <div class="card" style="margin-bottom: var(--space-4)">
            <p class="meta mt-0">
                "CSV columns, in order: project_code, plot_number, customer_lookup (an existing "
                "customer's ID number, phone, or email), payment_mode (full_cash / "
                "lipa_pole_pole_interest_free / lipa_pole_pole_interest_bearing), agreed_price, "
                "sale_date (YYYY-MM-DD), amount_paid (already repaid so far — 0 or blank if "
                "nothing's been paid yet, ignored for full_cash)."
            </p>
            <a
                href=template_href
                download="sales_template.csv"
                class="btn btn-secondary"
                style="margin-bottom: var(--space-3); display:inline-block;"
            >
                "Download CSV template"
            </a>

            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="bulk-sales-file">
                    {move || if parsing.get() { "Reading…" } else { "CSV file" }}
                </label>
                <input
                    id="bulk-sales-file"
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
                            let valid: Vec<(u32, BulkSaleRow)> = rows
                                .get()
                                .into_iter()
                                .filter_map(|r| r.result.ok().map(|input| (r.row, input)))
                                .collect();
                            let original_rows: Vec<u32> = valid.iter().map(|(row, _)| *row).collect();
                            let inputs: Vec<BulkSaleRow> =
                                valid.into_iter().map(|(_, input)| input).collect();
                            spawn_local(async move {
                                match api.bulk_create_sales(inputs).await {
                                    Ok(mut r) => {
                                        for e in r.errors.iter_mut() {
                                            if let Some(&orig) = original_rows.get(e.row as usize - 1) {
                                                e.row = orig;
                                            }
                                        }
                                        import_result.set(Some(r));
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
                                format!("Import {valid_count} sale(s)")
                            }
                        }}
                    </button>
                })
            }}

            {move || import_result.get().map(|r| view! {
                <div class="alert alert-warning">
                    <p>{format!("{} sale(s) imported.", r.created)}</p>
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
