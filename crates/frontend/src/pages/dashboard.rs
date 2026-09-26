use leptos::prelude::*;
use rust_decimal::prelude::ToPrimitive;

use crate::api::DashboardSummary;
use crate::auth::{use_api, use_auth, use_currency};
use crate::components::{BarChart, ChartPoint, DonutChart, DonutSegment, ErrorAlert, LineChart, LoadingState, StatCard};
use crate::format::{format_amount, format_money};
use domain::DashboardAnalytics;

#[component]
pub fn Dashboard() -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let currency = use_currency();

    let summary = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.dashboard_summary().await }
        }
    });
    let summary_for_health = summary;
    let analytics = LocalResource::new(move || {
        let api = api.clone();
        async move { api.dashboard_analytics().await }
    });

    let org_name = move || {
        auth.get()
            .map(|s| s.user.full_name)
            .unwrap_or_else(|| "there".to_string())
    };

    view! {
        <div class="page-header">
            <div>
                <h1>"Executive dashboard"</h1>
                <p>"Welcome back, " {org_name} ". Here's how the portfolio looks right now."</p>
            </div>
            // The figures below show bare numbers with no per-card
            // currency prefix — this is the one place the active
            // currency is stated, reactively, from `use_currency()`
            // (populated from Settings → General → Currency, not
            // hardcoded).
            <div class="currency-note">"Currency: " {move || currency.get()}</div>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading dashboard…" /> }>
            {move || {
                summary
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(s) => view! { <KpiStrip summary=s /> }.into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load the dashboard: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>

        <Suspense fallback=|| view! { <LoadingState label="Loading trends…" /> }>
            {move || {
                let currency = currency.get();
                analytics
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(move |result| match result {
                        Ok(a) => view! { <TrendCard analytics=a currency=currency.clone() /> }.into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load trends: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>

        <div class="analytics-grid-even">
            <Suspense fallback=|| view! { <div class="card"><LoadingState label="Loading…" /></div> }>
                {move || {
                    summary_for_health
                        .get()
                        .map(|wrapped| wrapped.take())
                        .map(|result| match result {
                            Ok(s) => view! { <PortfolioHealthCard summary=s /> }.into_any(),
                            Err(e) => view! { <ErrorAlert message=format!("{e}") /> }.into_any(),
                        })
                }}
            </Suspense>
            <Suspense fallback=|| view! { <div class="card"><LoadingState label="Loading…" /></div> }>
                {move || {
                    analytics
                        .get()
                        .map(|wrapped| wrapped.take())
                        .map(|result| match result {
                            Ok(a) => view! { <InventoryCard analytics=a /> }.into_any(),
                            Err(e) => view! { <ErrorAlert message=format!("{e}") /> }.into_any(),
                        })
                }}
            </Suspense>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading…" /> }>
            {move || {
                analytics
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(a) => view! { <ProjectBarCard analytics=a /> }.into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("{e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

/// The top-of-page snapshot — portfolio size, not performance. Trends
/// and health live in the chart cards below; this strip deliberately
/// stays small (no "Sales value" tile here any more — it only
/// duplicated the QTD/YTD figures now shown on the trend chart itself,
/// right next to the line that explains them).
#[component]
fn KpiStrip(summary: DashboardSummary) -> impl IntoView {
    view! {
        <div class="stat-grid">
            <StatCard label="Projects" value=summary.total_projects.to_string() />
            <StatCard label="Plots" value=summary.total_plots.to_string() />
            <StatCard label="Customers" value=summary.total_customers.to_string() />
            <StatCard
                label="Active loan book"
                value=format_amount(summary.active_loan_book)
                sub=format!("{} accounts", summary.active_loans_count)
            />
        </div>
    }
}

#[component]
fn PortfolioHealthCard(summary: DashboardSummary) -> impl IntoView {
    let total = summary.performing_count + summary.non_performing_count;
    let performing_pct = if total > 0 {
        (summary.performing_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let segments = vec![
        DonutSegment {
            label: "Performing".to_string(),
            value: summary.performing_amount.to_f64().unwrap_or(0.0).max(if summary.performing_count > 0 { 0.01 } else { 0.0 }),
            display: format!("{} · {}", summary.performing_count, format_amount(summary.performing_amount)),
            color: "#16a34a".to_string(),
        },
        DonutSegment {
            label: "Non-performing".to_string(),
            value: summary.non_performing_amount.to_f64().unwrap_or(0.0).max(if summary.non_performing_count > 0 { 0.01 } else { 0.0 }),
            display: format!("{} · {}", summary.non_performing_count, format_amount(summary.non_performing_amount)),
            color: "#dc2626".to_string(),
        },
    ];

    view! {
        <div class="card">
            <h3 class="mt-0">"Portfolio health"</h3>
            {if total == 0 {
                view! { <p class="meta">"No active loans yet."</p> }.into_any()
            } else {
                view! {
                    <DonutChart segments=segments center_label=format!("{performing_pct:.0}%") center_caption="performing".to_string() />
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn TrendCard(analytics: DashboardAnalytics, currency: String) -> impl IntoView {
    let ytd_value_f = analytics.ytd_sales_value.to_f64().unwrap_or(0.0);
    let prior_ytd_f = analytics.prior_ytd_sales_value.to_f64().unwrap_or(0.0);
    let delta_pct = if prior_ytd_f > 0.0 {
        Some(((ytd_value_f - prior_ytd_f) / prior_ytd_f) * 100.0)
    } else {
        None
    };

    let line_points: Vec<ChartPoint> = analytics
        .monthly_trend
        .iter()
        .map(|p| ChartPoint {
            label: p.period_label.clone(),
            value: p.sales_value.to_f64().unwrap_or(0.0),
            display: format_money(p.sales_value, &currency),
        })
        .collect();

    view! {
        <div class="card">
            <div class="dash-trend-header">
                <h3 class="mt-0">"Sales trend — last 12 months"</h3>
                <div class="dash-trend-kpis">
                    <div class="dash-kpi">
                        <span class="dash-kpi-label">"QTD"</span>
                        <span class="dash-kpi-value">{format_amount(analytics.qtd_sales_value)}</span>
                    </div>
                    <div class="dash-kpi">
                        <span class="dash-kpi-label">"YTD"</span>
                        <span class="dash-kpi-value">{format_amount(analytics.ytd_sales_value)}</span>
                        {delta_pct.map(|d| {
                            let (cls, arrow) = if d > 0.5 { ("up", "▲") } else if d < -0.5 { ("down", "▼") } else { ("flat", "•") };
                            view! { <span class=format!("kpi-delta {cls}")>{format!("{arrow} {:.0}% vs last year", d.abs())}</span> }
                        })}
                    </div>
                </div>
            </div>
            {if line_points.iter().all(|p| p.value == 0.0) {
                view! { <p class="meta">"No sales recorded in this period yet."</p> }.into_any()
            } else {
                view! { <LineChart points=line_points /> }.into_any()
            }}
        </div>
    }
}

#[component]
fn InventoryCard(analytics: DashboardAnalytics) -> impl IntoView {
    let total_plots: u32 = analytics.inventory_by_status.iter().map(|s| s.count).sum();
    let donut_segments: Vec<DonutSegment> = analytics
        .inventory_by_status
        .iter()
        .map(|s| DonutSegment {
            label: s.status_label.clone(),
            value: s.count as f64,
            display: format!("{} ({:.0}%)", s.count, if total_plots > 0 { s.count as f64 / total_plots as f64 * 100.0 } else { 0.0 }),
            color: s.status_color.clone(),
        })
        .collect();

    view! {
        <div class="card">
            <h3 class="mt-0">"Inventory — current position"</h3>
            {if donut_segments.is_empty() {
                view! { <p class="meta">"No plots yet."</p> }.into_any()
            } else {
                view! { <DonutChart segments=donut_segments center_label=total_plots.to_string() center_caption="total plots".to_string() /> }.into_any()
            }}
        </div>
    }
}

#[component]
fn ProjectBarCard(analytics: DashboardAnalytics) -> impl IntoView {
    let project_bars: Vec<ChartPoint> = analytics
        .sales_by_project
        .iter()
        .map(|p| ChartPoint {
            label: p.project_name.clone(),
            value: p.sales_value.to_f64().unwrap_or(0.0),
            display: format_amount(p.sales_value),
        })
        .collect();

    view! {
        <div class="card">
            <h3 class="mt-0">"Top projects by sales — YTD"</h3>
            {if project_bars.is_empty() {
                view! { <p class="meta">"No sales recorded this year yet."</p> }.into_any()
            } else {
                view! { <BarChart bars=project_bars /> }.into_any()
            }}
        </div>
    }
}
