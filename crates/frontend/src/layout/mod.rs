use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate};

use crate::auth::use_auth;

struct NavItem {
    href: &'static str,
    icon: &'static str,
    label: &'static str,
}

/// The phone's bottom tab bar only has room for a handful of tabs
/// before it turns into what the screenshot the app's owner sent
/// looked like: eight cramped icons, one label wrapping onto two
/// lines, the last tab clipped at the screen edge. Four is what
/// actually fits at a comfortable touch-target size — everything else
/// moves into the "More" sheet (`AppShell`'s `.nav-sheet`, below). The
/// sidebar (laptop/desktop) has room for all of them and stays flat —
/// see `all_nav_items`.
fn primary_nav_items() -> Vec<NavItem> {
    vec![
        NavItem { href: "/", icon: "\u{1F4CA}", label: "Home" },
        NavItem { href: "/projects", icon: "\u{1F3D8}\u{FE0F}", label: "Projects" },
        NavItem { href: "/customers", icon: "\u{1F464}", label: "Customers" },
        NavItem { href: "/quotations", icon: "\u{1F4C4}", label: "Quotes" },
    ]
}

/// "Platform" only appears for `is_platform_owner` accounts — everyone
/// else can't see it, and the backend enforces the same boundary on the
/// `/api/v1/platform/*` endpoints regardless (see
/// `crates/backend/src/routes/platform.rs`), so this is a convenience,
/// not the access control.
fn secondary_nav_items(is_platform_owner: bool) -> Vec<NavItem> {
    let mut items = vec![
        NavItem { href: "/approvals", icon: "\u{2705}", label: "Approvals" },
        NavItem { href: "/reports", icon: "\u{1F4C8}", label: "Reports" },
        NavItem { href: "/sales/import", icon: "\u{1F4E5}", label: "Import sales" },
    ];
    if is_platform_owner {
        items.push(NavItem {
            href: "/platform",
            icon: "\u{1F6E1}\u{FE0F}",
            label: "Platform",
        });
    }
    items
}

fn all_nav_items(is_platform_owner: bool) -> Vec<NavItem> {
    let mut items = primary_nav_items();
    items.extend(secondary_nav_items(is_platform_owner));
    items
}

/// Authenticated app layout: sidebar on laptop/desktop, top bar + bottom
/// tab bar on phone/tablet. Route protection lives here too — every
/// protected page's route `view` wraps its content in `<AppShell>`
/// (see `app.rs`), so the "logged in?" check and redirect happen once,
/// not per page.
#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();
    let location = use_location();
    let show_more = RwSignal::new(false);

    Effect::new(move |_| {
        if auth.get().is_none() {
            navigate("/login", Default::default());
        }
    });

    // Closes the "More" sheet once the route has actually changed,
    // rather than from the clicked link's own `on:click` — reacting
    // to navigation having happened, instead of to the click that
    // will (soon, asynchronously) cause it, sidesteps any risk of
    // synchronously mutating the DOM while leptos_router's own
    // window-level click listener (`handle_anchor_click`) is still in
    // the middle of handling that same click.
    Effect::new(move |_| {
        location.pathname.track();
        show_more.set(false);
    });

    let initials = move || {
        auth.get()
            .map(|s| {
                s.user
                    .full_name
                    .split_whitespace()
                    .filter_map(|w| w.chars().next())
                    .take(2)
                    .collect::<String>()
                    .to_uppercase()
            })
            .unwrap_or_default()
    };
    let full_name = move || auth.get().map(|s| s.user.full_name).unwrap_or_default();
    let is_platform_owner =
        move || auth.get().map(|s| s.user.is_platform_owner).unwrap_or(false);

    // No <Show when=is_authenticated> around this: the Effect above already
    // redirects to /login the moment `auth` is None, and gating the whole
    // subtree on a signal here would require `children` to be `Fn` (called
    // on every re-render) rather than `FnOnce` (called once at setup),
    // which the mock-auth phase doesn't need — this is UI-layer routing,
    // not the real access control (that's the backend's job, see
    // docs/10-database-and-security-design.md).
    view! {
        <div class="app-shell">
            <aside class="sidebar">
                <div class="sidebar-brand">
                    <span class="logo-mark">"R"</span>
                    <span>"Real Estate Manager"</span>
                </div>
                <nav class="sidebar-nav">
                    {move || {
                        all_nav_items(is_platform_owner())
                            .into_iter()
                            .map(|item| {
                                view! {
                                    <A href=item.href exact=item.href == "/">
                                        <span>{item.icon}</span>
                                        <span>{item.label}</span>
                                    </A>
                                }
                            })
                            .collect_view()
                    }}
                </nav>
            </aside>

            <header class="topbar">
                <div class="topbar-brand">
                    <span class="logo-mark">"R"</span>
                    <span>"Real Estate Manager"</span>
                </div>
                <div class="topbar-user">
                    <span>{full_name}</span>
                    <span class="avatar">{initials}</span>
                </div>
            </header>

            <main class="main-content">{children()}</main>

            <nav class="bottom-nav">
                {move || {
                    primary_nav_items()
                        .into_iter()
                        .map(|item| {
                            view! {
                                <A href=item.href exact=item.href == "/">
                                    <span class="icon">{item.icon}</span>
                                    <span>{item.label}</span>
                                </A>
                            }
                        })
                        .collect_view()
                }}
                <button
                    type="button"
                    class="bottom-nav-more"
                    class:active=move || show_more.get()
                    on:click=move |_| show_more.update(|v| *v = !*v)
                >
                    <span class="icon">"\u{22EF}"</span>
                    <span>"More"</span>
                </button>
            </nav>

            <Show when=move || show_more.get()>
                // Closes on a genuine backdrop click (target is the
                // backdrop itself) without `stop_propagation()` on the
                // sheet to protect that — `stop_propagation()` there
                // would also stop every click inside the sheet
                // (including on its nav links) from ever reaching
                // leptos_router's own window-level click listener,
                // which needs the *unstopped* event to recognise the
                // click and turn it into a client-side route change.
                // Blocked from seeing it, the browser fell through to
                // a real full-page navigation on every link in this
                // sheet — full reloads wipe the in-memory session
                // (`auth.rs`), bouncing back to /login. This was a
                // real, reproduced bug, not a hypothetical.
                <div
                    class="nav-sheet-backdrop"
                    on:click=move |ev| {
                        use leptos::wasm_bindgen::JsCast;
                        let target_is_backdrop = ev
                            .target()
                            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                            .is_some_and(|t| t.class_list().contains("nav-sheet-backdrop"));
                        if target_is_backdrop {
                            show_more.set(false);
                        }
                    }
                >
                    <div class="nav-sheet">
                        <div class="nav-sheet-handle"></div>
                        {move || {
                            secondary_nav_items(is_platform_owner())
                                .into_iter()
                                .map(|item| {
                                    view! {
                                        <A href=item.href>
                                            <span class="icon">{item.icon}</span>
                                            <span>{item.label}</span>
                                        </A>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}
