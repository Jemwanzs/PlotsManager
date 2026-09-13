use leptos::prelude::*;

use crate::api::DashboardSummary;
use crate::auth::{use_api, use_auth};
use crate::components::{ErrorAlert, LoadingState, StatCard};
use crate::format::format_kes;

#[component]
pub fn Dashboard() -> impl IntoView {
    let api = use_api();
    let auth = use_auth();

    let summary = LocalResource::new(move || {
        let api = api.clone();
        async move { api.dashboard_summary().await }
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
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading dashboard…" /> }>
            {move || {
                summary
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(s) => view! { <DashboardContent summary=s /> }.into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load the dashboard: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn DashboardContent(summary: DashboardSummary) -> impl IntoView {
    let arrears_total = summary.performing_count + summary.non_performing_count;
    let arrears_rate = if arrears_total > 0 {
        format!(
            "{:.0}% performing",
            (summary.performing_count as f64 / arrears_total as f64) * 100.0
        )
    } else {
        "No active loans".to_string()
    };

    view! {
        <div class="stat-grid">
            <StatCard label="Projects" value=summary.total_projects.to_string() />
            <StatCard label="Plots" value=summary.total_plots.to_string() />
            <StatCard label="Customers" value=summary.total_customers.to_string() />
            <StatCard
                label="Sales value"
                value=format_kes(summary.total_sales_value)
                sub=format!("{} sales", summary.total_sales_count)
            />
            <StatCard
                label="Active loan book"
                value=format_kes(summary.active_loan_book)
                sub=format!("{} accounts", summary.active_loans_count)
            />
        </div>

        <div class="card">
            <h2>"Portfolio health"</h2>
            <p>{arrears_rate}</p>
            <div class="stat-grid" style="margin-bottom: 0">
                <StatCard
                    label="Performing"
                    value=summary.performing_count.to_string()
                    sub=format_kes(summary.performing_amount)
                />
                <StatCard
                    label="Non-performing"
                    value=summary.non_performing_count.to_string()
                    sub=format_kes(summary.non_performing_amount)
                />
            </div>
        </div>
    }
}
