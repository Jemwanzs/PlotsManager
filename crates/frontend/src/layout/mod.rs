use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate};

use crate::auth::use_auth;
use crate::icons::{Icon, IconName};
use crate::theme::{use_theme, Theme};

struct NavItem {
    href: &'static str,
    icon: IconName,
    label: &'static str,
}

/// One destination inside an expanded sidebar module — plain text, no
/// icon of its own (the module's icon already identifies the section;
/// repeating a smaller icon per child just adds noise, see the reference
/// screenshot this nav's accordion shape was modelled on). `Copy`: both
/// fields are `&'static str`, so cloning a `Vec<NavChild>` into a
/// reactive closure below is free and sidesteps borrowing a `NavGroup`
/// that doesn't outlive the `.map()` iteration building it.
#[derive(Clone, Copy)]
struct NavChild {
    href: &'static str,
    label: &'static str,
}

/// A collapsible sidebar module: click the row to expand/collapse its
/// children (see `AppShell`'s `expanded_group` signal), rather than the
/// row itself navigating anywhere — matches the reference sidebar this
/// was modelled on, where a module header is purely a toggle and every
/// real destination lives one level down.
struct NavGroup {
    icon: IconName,
    label: &'static str,
    children: Vec<NavChild>,
}

/// The phone's bottom tab bar only has room for a handful of tabs
/// before it turns into what the screenshot the app's owner sent
/// looked like: eight cramped icons, one label wrapping onto two
/// lines, the last tab clipped at the screen edge. Four is what
/// actually fits at a comfortable touch-target size — everything else
/// moves into the "More" sheet, below. The desktop sidebar has room for
/// all of them and gets its own richer, expandable structure — see
/// `sidebar_nav_groups`.
fn primary_nav_items() -> Vec<NavItem> {
    vec![
        NavItem { href: "/", icon: IconName::Home, label: "Home" },
        NavItem { href: "/projects", icon: IconName::Projects, label: "Projects" },
        NavItem { href: "/customers", icon: IconName::Customers, label: "Customers" },
        NavItem { href: "/quotations", icon: IconName::Quotes, label: "Quotes" },
    ]
}

/// "Platform" only appears for `is_platform_owner` accounts — everyone
/// else can't see it, and the backend enforces the same boundary on the
/// `/api/v1/platform/*` endpoints regardless (see
/// `crates/backend/src/routes/platform.rs`), so this is a convenience,
/// not the access control. The mobile "More" sheet stays flat (one tap
/// to a module's main page) rather than growing its own accordion —
/// each module's in-page tabs (Approvals, Reports, Quotations, ...
/// already have them; Finance's overview page links out to its loan
/// accounts list) are how a phone reaches the same sub-destinations the
/// desktop sidebar exposes directly.
fn secondary_nav_items(is_platform_owner: bool) -> Vec<NavItem> {
    let mut items = vec![
        NavItem { href: "/approvals", icon: IconName::Approvals, label: "Approvals" },
        NavItem { href: "/finance", icon: IconName::Finance, label: "Finance" },
        NavItem { href: "/reports", icon: IconName::Reports, label: "Reports" },
        NavItem { href: "/sales/import", icon: IconName::Import, label: "Import sales" },
        NavItem { href: "/settings", icon: IconName::Settings, label: "Settings" },
    ];
    if is_platform_owner {
        items.push(NavItem {
            href: "/platform",
            icon: IconName::Platform,
            label: "Platform",
        });
    }
    items
}

/// The desktop sidebar's module list — each one expands to at least two
/// real destinations instead of a single flat link, so a module reads
/// as a section of the product rather than one page. Wherever a second
/// destination didn't already exist as its own route, a lightweight one
/// was added rather than inventing a placeholder: `/activity` (recent
/// sales), the Finance module's two pages, and small query-param-driven
/// pre-filters on pages that already had the underlying filter UI
/// (Approvals, Reports, Quotations) or nearly did (Customers' bulk
/// import). "Platform" is deliberately exempt — it's a single-purpose,
/// owner-only cross-tenant admin utility, not a business module with
/// natural sub-areas, so it's a direct link with no chevron.
fn sidebar_nav_groups(is_platform_owner: bool) -> Vec<NavGroup> {
    let mut groups = vec![
        NavGroup {
            icon: IconName::Home,
            label: "Home",
            children: vec![
                NavChild { href: "/", label: "Overview" },
                NavChild { href: "/activity", label: "Recent activity" },
            ],
        },
        NavGroup {
            icon: IconName::Projects,
            label: "Projects",
            children: vec![
                NavChild { href: "/projects", label: "All projects" },
                NavChild { href: "/projects/new", label: "Add project" },
            ],
        },
        NavGroup {
            icon: IconName::Customers,
            label: "Customers",
            children: vec![
                NavChild { href: "/customers", label: "All customers" },
                NavChild { href: "/customers/new", label: "Add customer" },
                NavChild { href: "/customers?import=1", label: "Import customers" },
            ],
        },
        NavGroup {
            icon: IconName::Quotes,
            label: "Quotes",
            children: vec![
                NavChild { href: "/quotations", label: "All quotations" },
                NavChild { href: "/quotations?filter=active", label: "Active quotations" },
            ],
        },
        NavGroup {
            icon: IconName::Approvals,
            label: "Approvals",
            children: vec![
                NavChild { href: "/approvals", label: "Pending approvals" },
                NavChild { href: "/approvals?tab=history", label: "Approval history" },
            ],
        },
        NavGroup {
            icon: IconName::Finance,
            label: "Finance",
            children: vec![
                NavChild { href: "/finance", label: "Overview" },
                NavChild { href: "/finance/loan-accounts", label: "Loan accounts" },
            ],
        },
        NavGroup {
            icon: IconName::Reports,
            label: "Reports",
            children: vec![
                NavChild { href: "/reports", label: "Sales report" },
                NavChild { href: "/reports?tab=inventory", label: "Inventory report" },
                NavChild { href: "/reports?tab=agents", label: "Agent performance" },
            ],
        },
        NavGroup {
            icon: IconName::Import,
            label: "Data import",
            children: vec![
                NavChild { href: "/sales/import", label: "Import sales" },
                NavChild { href: "/customers?import=1", label: "Import customers" },
            ],
        },
        NavGroup {
            icon: IconName::Settings,
            label: "Settings",
            children: vec![
                NavChild { href: "/settings", label: "General settings" },
                NavChild { href: "/settings#numbering", label: "Numbering configuration" },
                NavChild { href: "/settings/users", label: "Users & access" },
                NavChild { href: "/settings/branches", label: "Branches" },
                NavChild { href: "/settings/roles", label: "Roles & permissions" },
                NavChild { href: "/account/change-password", label: "Change my password" },
            ],
        },
    ];
    if is_platform_owner {
        groups.push(NavGroup {
            icon: IconName::Platform,
            label: "Platform",
            children: vec![NavChild { href: "/platform", label: "Organizations" }],
        });
    }
    groups
}

/// The path portion of an `href` that may carry a query string or hash
/// (`"/customers?import=1"`, `"/settings#numbering"`) — matched against
/// the current route to decide which module should auto-expand.
fn path_only(href: &str) -> &str {
    href.split(['?', '#']).next().unwrap_or(href)
}

fn child_matches_path(current_path: &str, child_href: &str) -> bool {
    let child_path = path_only(child_href);
    if child_path == "/" {
        current_path == "/"
    } else {
        current_path == child_path || current_path.starts_with(&format!("{child_path}/"))
    }
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

    // Single-open accordion: which sidebar module (by its `label`, a
    // stable `&'static str`) is currently expanded. Re-derived from the
    // current route on every navigation, so landing on `/projects/new`
    // via a bookmark or a link from elsewhere still opens "Projects"
    // instead of leaving every module collapsed.
    let expanded_group: RwSignal<Option<&'static str>> = RwSignal::new(None);

    let navigate_for_login_check = navigate.clone();
    Effect::new(move |_| {
        if auth.get().is_none() {
            navigate_for_login_check("/login", Default::default());
        }
    });

    // A temporary password (admin-created account, or an admin-issued
    // reset) must be changed before anything else — see
    // `crates/frontend/src/pages/change_password.rs`. Re-evaluates on
    // every navigation and every `auth` change, so it both catches a
    // user trying to click away from the gate and clears itself the
    // moment `change_password` succeeds and sets a fresh session.
    let navigate_for_password_gate = navigate.clone();
    Effect::new(move |_| {
        let path = location.pathname.get();
        let must_change = auth.get().map(|s| s.user.must_change_password).unwrap_or(false);
        if must_change && path != "/account/change-password" {
            navigate_for_password_gate("/account/change-password", Default::default());
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

    let is_platform_owner =
        move || auth.get().map(|s| s.user.is_platform_owner).unwrap_or(false);

    // Auto-expands whichever module owns the current route. Depends on
    // `is_platform_owner` too (not just the path) so the derived groups
    // list — and therefore which one matches — stays correct across
    // sign-in/out on the same tab.
    Effect::new(move |_| {
        let path = location.pathname.get();
        let owner = is_platform_owner();
        let matched = sidebar_nav_groups(owner)
            .into_iter()
            .find(|g| g.children.iter().any(|c| child_matches_path(&path, c.href)))
            .map(|g| g.label);
        expanded_group.set(matched);
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
    let theme = use_theme();
    let theme_label = move || match theme.get() {
        Theme::Light => "Dark mode",
        Theme::Dark => "Light mode",
    };
    let theme_icon = move || match theme.get() {
        Theme::Light => IconName::Moon,
        Theme::Dark => IconName::Sun,
    };
    let toggle_theme = move |_| theme.update(|t| *t = t.toggled());

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
                        sidebar_nav_groups(is_platform_owner())
                            .into_iter()
                            .map(|group| {
                                let icon = group.icon;
                                let label = group.label;
                                let children = group.children;
                                let is_open = move || expanded_group.get() == Some(label);
                                if children.len() == 1 {
                                    let href = children[0].href;
                                    return view! {
                                        <A href=href attr:class="nav-group-toggle">
                                            <span class="icon"><Icon name=icon /></span>
                                            <span>{label}</span>
                                        </A>
                                    }
                                        .into_any();
                                }
                                view! {
                                    <div class="nav-group">
                                        <button
                                            type="button"
                                            class="nav-group-toggle"
                                            class:open=is_open
                                            on:click=move |_| {
                                                expanded_group
                                                    .update(|e| {
                                                        *e = if *e == Some(label) { None } else { Some(label) };
                                                    })
                                            }
                                        >
                                            <span class="icon"><Icon name=icon /></span>
                                            <span>{label}</span>
                                            <span class="chevron" class:open=is_open>
                                                <Icon name=IconName::Chevron />
                                            </span>
                                        </button>
                                        // Rendered once, visibility toggled reactively via a
                                        // CSS class rather than mounted/unmounted through
                                        // `<Show>` — the children never change, only whether
                                        // they're visible, so there's no need for a `Fn`
                                        // closure that re-clones `children` on every toggle.
                                        <div class="nav-group-children" class:hidden=move || !is_open()>
                                            {children
                                                .into_iter()
                                                .map(|child| {
                                                    view! { <A href=child.href>{child.label}</A> }
                                                })
                                                .collect_view()}
                                        </div>
                                    </div>
                                }
                                    .into_any()
                            })
                            .collect_view()
                    }}
                </nav>
                <button type="button" class="theme-toggle" on:click=toggle_theme>
                    <span class="icon">{move || view! { <Icon name=theme_icon() /> }}</span>
                    <span>{theme_label}</span>
                </button>
            </aside>

            <header class="topbar">
                <div class="topbar-brand">
                    <span class="logo-mark">"R"</span>
                    <span>"Real Estate Manager"</span>
                </div>
                <div class="topbar-user">
                    <button
                        type="button"
                        class="theme-toggle-icon"
                        aria-label="Toggle dark mode"
                        on:click=toggle_theme
                    >
                        {move || view! { <Icon name=theme_icon() /> }}
                    </button>
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
                                    <span class="icon"><Icon name=item.icon /></span>
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
                    <span class="icon"><Icon name=IconName::More /></span>
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
                                            <span class="icon"><Icon name=item.icon /></span>
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
