use chrono::NaiveDate;
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use uuid::Uuid;

use crate::auth::{use_api, use_currency};
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatCard, StatusBadge};
use crate::format::{format_money, format_payment_mode};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Sales,
    Inventory,
    Agents,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Sales => "Sales",
            Tab::Inventory => "Inventory",
            Tab::Agents => "Agent performance",
        }
    }
}

#[component]
pub fn Reports() -> impl IntoView {
    // `/reports?tab=inventory` / `?tab=agents` (the sidebar's Reports
    // sub-items) pre-select a tab; still switchable afterward like any
    // other in-page filter.
    let query = use_query_map();
    let initial_tab = match query.get_untracked().get("tab").as_deref() {
        Some("inventory") => Tab::Inventory,
        Some("agents") => Tab::Agents,
        _ => Tab::Sales,
    };
    let tab = RwSignal::new(initial_tab);

    view! {
        <div class="page-header">
            <div>
                <h1>"Reports"</h1>
                <p>"Sales, inventory and agent performance, drawn from your live data."</p>
            </div>
        </div>

        <div class="filter-tabs">
            {[Tab::Sales, Tab::Inventory, Tab::Agents]
                .into_iter()
                .map(|t| {
                    view! {
                        <button
                            type="button"
                            class="filter-tab"
                            class:active=move || tab.get() == t
                            on:click=move |_| tab.set(t)
                        >
                            {t.label()}
                        </button>
                    }
                })
                .collect_view()}
        </div>

        {move || match tab.get() {
            Tab::Sales => view! { <SalesReportTab /> }.into_any(),
            Tab::Inventory => view! { <InventoryReportTab /> }.into_any(),
            Tab::Agents => view! { <AgentReportTab /> }.into_any(),
        }}
    }
}

fn parse_date(s: String) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

#[component]
fn SalesReportTab() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();
    let project_filter = RwSignal::new(String::new());
    let from_filter = RwSignal::new(String::new());
    let to_filter = RwSignal::new(String::new());

    let projects = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.list_projects().await }
        }
    });

    let report = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            let project_id = Uuid::parse_str(project_filter.get().trim()).ok();
            let from = parse_date(from_filter.get());
            let to = parse_date(to_filter.get());
            async move { api.sales_report(project_id, None, from, to).await }
        }
    });

    view! {
        <div class="card" style="margin-bottom: var(--space-4)">
            <div style="display:flex; gap: var(--space-3); flex-wrap: wrap; align-items: flex-end;">
                <div class="field" style="margin-bottom: 0;">
                    <label for="report-project">"Project"</label>
                    <select
                        id="report-project"
                        prop:value=project_filter
                        on:change=move |ev| project_filter.set(event_target_value(&ev))
                    >
                        <option value="">"All projects"</option>
                        <Suspense fallback=|| ()>
                            {move || {
                                projects
                                    .get()
                                    .map(|wrapped| wrapped.take())
                                    .map(|result| match result {
                                        Ok(list) => list
                                            .into_iter()
                                            .map(|p| view! { <option value=p.id.to_string()>{p.name}</option> })
                                            .collect_view()
                                            .into_any(),
                                        Err(_) => ().into_any(),
                                    })
                            }}
                        </Suspense>
                    </select>
                </div>
                <div class="field" style="margin-bottom: 0;">
                    <label for="report-from">"From"</label>
                    <input
                        id="report-from"
                        type="date"
                        prop:value=from_filter
                        on:change=move |ev| from_filter.set(event_target_value(&ev))
                    />
                </div>
                <div class="field" style="margin-bottom: 0;">
                    <label for="report-to">"To"</label>
                    <input
                        id="report-to"
                        type="date"
                        prop:value=to_filter
                        on:change=move |ev| to_filter.set(event_target_value(&ev))
                    />
                </div>
            </div>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading sales report…" /> }>
            {move || {
                report
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(r) if r.rows.is_empty() => view! {
                            <EmptyState
                                icon="\u{1F4CB}"
                                title="No sales in range"
                                detail="Adjust the filters above, or come back once a plot has been reserved."
                            />
                        }
                            .into_any(),
                        Ok(r) => view! {
                            <div class="stat-grid">
                                <StatCard label="Sales" value=r.total_count.to_string() />
                                <StatCard label="Total value" value=format_money(r.total_value, &currency.get()) />
                            </div>
                            <div class="card">
                                <div class="table-scroll">
                                    <table class="data-table">
                                        <thead>
                                            <tr>
                                                <th>"Date"</th>
                                                <th>"Project"</th>
                                                <th>"Plot"</th>
                                                <th>"Customer"</th>
                                                <th>"Agent"</th>
                                                <th>"Payment mode"</th>
                                                <th>"Price"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {r.rows
                                                .into_iter()
                                                .map(|row| {
                                                    view! {
                                                        <tr>
                                                            <td>{row.created_at.format("%b %d, %Y").to_string()}</td>
                                                            <td>{row.project_name}</td>
                                                            <td>{row.plot_number}</td>
                                                            <td>{row.customer_name}</td>
                                                            <td>{row.agent_name.unwrap_or_else(|| "\u{2014}".to_string())}</td>
                                                            <td>{format_payment_mode(row.payment_mode)}</td>
                                                            <td>{format_money(row.agreed_price, &currency.get())}</td>
                                                        </tr>
                                                    }
                                                })
                                                .collect_view()}
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        }
                            .into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load the sales report: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn InventoryReportTab() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let report = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.inventory_report().await }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading inventory report…" /> }>
            {move || {
                report
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(r) if r.by_project.is_empty() => view! {
                            <EmptyState
                                icon="\u{1F3D8}\u{FE0F}"
                                title="No plots yet"
                                detail="Inventory will show up here once a project has plots."
                            />
                        }
                            .into_any(),
                        Ok(r) => view! {
                            <div class="card-grid">
                                {r.by_project
                                    .into_iter()
                                    .map(|project| {
                                        view! {
                                            <div class="card">
                                                <h3 class="mt-0">{project.project_name}</h3>
                                                <p class="meta mt-0">{project.total_plots} " plots"</p>
                                                <div class="table-scroll">
                                                    <table class="data-table">
                                                        <thead>
                                                            <tr>
                                                                <th>"Status"</th>
                                                                <th>"Count"</th>
                                                                <th>"Value"</th>
                                                            </tr>
                                                        </thead>
                                                        <tbody>
                                                            {project.by_status
                                                                .into_iter()
                                                                .map(|s| {
                                                                    view! {
                                                                        <tr>
                                                                            <td>
                                                                                <StatusBadge label=s.status_label color=s.status_color />
                                                                            </td>
                                                                            <td>{s.count}</td>
                                                                            <td>{format_money(s.value, &currency.get())}</td>
                                                                        </tr>
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </tbody>
                                                    </table>
                                                </div>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load the inventory report: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn AgentReportTab() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();
    let from_filter = RwSignal::new(String::new());
    let to_filter = RwSignal::new(String::new());

    let report = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            let from = parse_date(from_filter.get());
            let to = parse_date(to_filter.get());
            async move { api.agent_performance_report(from, to).await }
        }
    });

    view! {
        <div class="card" style="margin-bottom: var(--space-4)">
            <div style="display:flex; gap: var(--space-3); flex-wrap: wrap; align-items: flex-end;">
                <div class="field" style="margin-bottom: 0;">
                    <label for="agent-report-from">"From"</label>
                    <input
                        id="agent-report-from"
                        type="date"
                        prop:value=from_filter
                        on:change=move |ev| from_filter.set(event_target_value(&ev))
                    />
                </div>
                <div class="field" style="margin-bottom: 0;">
                    <label for="agent-report-to">"To"</label>
                    <input
                        id="agent-report-to"
                        type="date"
                        prop:value=to_filter
                        on:change=move |ev| to_filter.set(event_target_value(&ev))
                    />
                </div>
            </div>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading agent performance…" /> }>
            {move || {
                report
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(r) if r.rows.is_empty() => view! {
                            <EmptyState
                                icon="\u{1F464}"
                                title="No activity in range"
                                detail="Adjust the filters above, or come back once an agent has a sale or quotation."
                            />
                        }
                            .into_any(),
                        Ok(r) => view! {
                            <div class="card">
                                <div class="table-scroll">
                                    <table class="data-table">
                                        <thead>
                                            <tr>
                                                <th>"Agent"</th>
                                                <th>"Sales"</th>
                                                <th>"Sales value"</th>
                                                <th>"Quotations sent"</th>
                                                <th>"Accepted"</th>
                                                <th>"Conversion"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {r.rows
                                                .into_iter()
                                                .map(|row| {
                                                    let conversion = if row.quotations_sent > 0 {
                                                        format!(
                                                            "{:.0}%",
                                                            (row.quotations_accepted as f64 / row.quotations_sent as f64) * 100.0,
                                                        )
                                                    } else {
                                                        "\u{2014}".to_string()
                                                    };
                                                    view! {
                                                        <tr>
                                                            <td>{row.agent_name}</td>
                                                            <td>{row.sales_count}</td>
                                                            <td>{format_money(row.sales_value, &currency.get())}</td>
                                                            <td>{row.quotations_sent}</td>
                                                            <td>{row.quotations_accepted}</td>
                                                            <td>{conversion}</td>
                                                        </tr>
                                                    }
                                                })
                                                .collect_view()}
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        }
                            .into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load agent performance: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
