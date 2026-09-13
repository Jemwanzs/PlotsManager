use leptos::prelude::*;

use crate::components::EmptyState;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div style="padding: var(--space-6) 0">
            <EmptyState icon="\u{1F9ED}" title="Page not found" detail="Check the link, or use the navigation to get back on track." />
        </div>
    }
}
