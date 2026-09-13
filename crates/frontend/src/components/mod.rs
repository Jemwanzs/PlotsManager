use leptos::prelude::*;

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
