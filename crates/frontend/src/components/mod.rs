use leptos::prelude::*;

use crate::icons::{Icon, IconName};

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
