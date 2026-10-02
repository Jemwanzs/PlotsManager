# 14 — Development Roadmap

Two roadmaps from the original scoping conversation are merged here: the
platform-wide phases (1–7) and the payments-specific delivery sequence
(A–D), which nests inside phases 5–7.

## Current status (2026-09-26)

**This section went stale for a while — it described a pre-deployment,
mock-only frontend as of 2026-09-15, and nobody had gone back to
correct Phases 2/3/5/6 against what had actually shipped since.
Corrected below after an actual code audit, not just a memory of what
was planned.**

**Architecture (final, live): Frontend → Rust API → PostgreSQL, all on
Railway** (project `c7bee255-492d-40b6-af50-30374625b279` — frontend at
`jm-plots.up.railway.app`, backend at
`backend-production-3d3d.up.railway.app`). `crates/backend` (Axum) owns
authentication/authorization/tenant isolation; Postgres lives on
Railway with Row-Level Security as defense-in-depth, not the
enforcement point. See [10](10-database-and-security-design.md) and
[12](12-api-and-integration-design.md).

The Cargo workspace is `domain` (shared types, request/response DTOs,
the permission registry, and status-color helpers `backend` and
`frontend` both use), `backend` (Axum — real, deployed routes across
every area below), and `frontend` (Leptos CSR, deployed, talking to the
real backend through `crates/frontend/src/api/http.rs`). The in-memory
mock behind `crates/frontend/src/api/mock.rs` still exists and is kept
in step with the real backend, but only as a local-dev/offline
convenience now, not the frontend's data source. `database/migrations/`
(0001 through 0034 as of this correction) holds the schema, RLS
policies, and every backfill, applied automatically by the backend on
boot.

**What's actually left**, now that Phases 2/3/5/6A/6B/6C are
substantially built (see each phase's own corrected status below):
Phase 4 (AI-assisted plan conversion), Phase 6D (real payment-provider
integrations — mobile-money/banking, automated reconciliation, a
customer self-service portal), Phase 7's GIS/satellite mapping and
SMS/email/WhatsApp/accounting integrations, a real plan-selection UI
for signup (today's form only takes a free-text "preferred package"
string — see Platform billing's own note), the Captured → Verified → Posted
payment approval lifecycle (an open product question, not a known
gap — see Phase 6's own note), and the remaining 22 of 24 backend
route files' migration to the least-privilege `app_user` RLS role
(infrastructure + a two-file pilot shipped 2026-10-01 — see docs/10's
own note). Per the 2026-09-26 decision, every integration in that list
is deliberately sequenced *after* building the settings-driven
plug-in-configuration infrastructure they'll all be wired through —
see the new section below. Notifications/work queues and the
customer-360 financial view shipped 2026-09-27/28 — see their own
entries below.

## Phase 1 — Discovery and Legacy Analysis
Analyse the Excel/VBA system, extract business rules, document current
workflows, identify migration requirements, produce full specs.
**Status: complete.** 58 VBA modules exported and analysed — see
[02](02-existing-vba-system-analysis.md) for the full data model,
numbering rules, workflow behavior, security posture, and a gap-analysis
table mapping legacy behavior to every affected spec doc (03, 04, 05, 08,
09, 10, 11, 13). One open product decision surfaced: whether to carry
forward the legacy customer feedback/ratings module, currently unspecified
anywhere else in `docs/`.

## Phase 2 — Platform Foundation
Multi-tenant architecture; organisation settings; users, roles,
permissions; projects and plot register; documents and audit logs;
configurable numbering.
**Status (corrected 2026-09-26): deployed and live on Railway, frontend
wired to the real backend, not the mock.** Login (Argon2 + JWT),
projects/plots, customers, documents (`routes/documents.rs`), and full
role/permission management (`crates/domain/src/permissions.rs`'s
registry, backfilled per-migration onto every existing role whenever a
new permission is added) are all real, scoped by `organization_id`, and
live in production. Org creation/signup is a real, gated flow — a new
sign-up lands in `pending_approval` and needs Platform Owner
approval/rejection before it can trial (`0015_tenant_onboarding.sql`,
`routes/platform.rs`; the approve/reject UI itself was a bug fixed
2026-09-26, see this doc's earlier entry). The mock API layer
(`crates/frontend/src/api/mock.rs`) still exists and is kept in lock-
step with the real backend, but as a local-dev/offline convenience, not
the frontend's actual data source. **Numbering config (corrected
2026-09-28 — my own 2026-09-26 note here was wrong): already fully
built**, not a gap — prefix, "include year", "include project code"
(plots only), digit padding, and the next number to issue are all
admin-editable per entity type from Settings, with a live preview
(`domain::format_sequence_number`, `pages/settings.rs`) computed from
the same fields the backend actually uses to generate the next real
number. The least-privilege RLS-subject Postgres role — see docs/10's
own note — now has real infrastructure (2026-10-01): `app_user`
(`database/migrations/0037_app_user_role.sql`, `nologin nobypassrls`,
switched into per-request via `set local role` in
`crates/backend/src/extractors.rs::TenantTx` — no new credentials, no
second connection pool) and two of 24 route files
(`customers.rs`, `sales.rs`) actually running under it, verified
against a local Postgres that RLS blocks cross-tenant rows even with
the application's own `where organization_id = $1` deliberately
stripped out. The other 22 files are unconverted and still work
exactly as before (correct today via that same app-level filter, just
without RLS as a second layer yet) — each is its own later, low-risk,
one-file pass, same shape as this one.

## Phase 3 — Interactive Maps
Upload project plans; manual polygon drawing; plot-to-map linking;
colour-coded statuses; search/filter/pan/zoom; map versioning and
approvals.
**Status (corrected 2026-09-26 — previously misread as "not started"):
substantially built.** `routes/project_map.rs` + `pages/project_detail.rs::
MapCanvas` — image upload, freehand polygon drawing/deletion,
status-coloured shapes, "Create Plot"/"Link Existing Plot"/"Unlink"
for a drawn shape (a shape no longer needs a plot picked up front —
see `domain::MapFeature`'s module docs), all authenticated and
permission-gated (`PERM_PLOTS_MAP_UPLOAD/EDIT_BOUNDARIES/LINK`). A
search/status-filter/zoom toolbar shipped 2026-09-30 (pure client-side,
`pages/project_detail.rs::MapCanvas` — search matches a shape's label
or its linked plot number, status filters to plots in that state, both
dim non-matching shapes rather than hiding them; zoom scales the image
to an explicit pixel width off its normal "fit" size, and the wrapping
container's native scroll handles panning once zoomed past its
`max-height`/`max-width`, so no drag-to-pan handler was needed). **Map
versioning shipped 2026-10-02**: real draft/published lifecycle
(`database/migrations/0038_project_map_versions.sql`, replacing the
single mutable row `0009_project_map.sql` deliberately cut this down
to) — editing always edits a draft, invisible to anyone else, and an
explicit "Publish" promotes it to the version everyone sees; every
prior published version becomes permanent, read-only `superseded`
history, viewable via a "Show version history" panel. Deliberately not
built in this pass (per the explicit choice made when scoping it):
polygon split/merge tools and a side-by-side comparison view against
the originally uploaded document — both separate, later work if ever
wanted.

## Phase 4 — AI-Assisted Plan Conversion
Image enhancement; OCR; boundary detection; plot-number recognition;
confidence scores; exception handling and human correction.
**Status: not started — deliberately sequenced after Phase 3.**

## Phase 5 — Sales and Customer Management
Leads/prospects; plot selection; holds/reservations/bookings; quotations/
offer letters; sales agreements; customer 360°; agent assignment/
commissions.
**Status (corrected 2026-09-26): real backend + frontend throughout,
not "still mock."** Leads pipeline (`LeadStage`,
`customers:leads.update`), plot reservation/booking (`POST
/api/v1/sales`, Full Cash and both Lipa Pole Pole modes), quotations/
offer letters (draft → send → accept/reject, `routes/quotations.rs`,
with a price-approval gate below a configured minimum —
`routes/approvals.rs`), and agent commissions (per-project override
over an org-wide default, accrued at sale creation, voided on
cancel/repossess — `routes/sales.rs`, `0032_agent_commissions.sql`)
are all built and wired end-to-end. **Customer 360° view (corrected
2026-09-27 — my own 2026-09-26 note here was wrong about document
attachments, which were already built): now real.** `CustomerDetail`
carries a cross-sale financial rollup (`total_agreed_value`/
`total_paid`/`total_outstanding`, a Full Cash sale counted as paid in
full since it has no loan account to sum) and each sale's own
paid/outstanding when it has one, shown as a stat-card row on the
customer page (`routes/customers.rs::get_customer`,
`pages/customer_detail.rs`); document attachments were already wired
via the generic `DocumentsPanel` (`entity_type: Customer`) before this
correction, not missing. **Communications log: now built (2026-09-28)**
— an append-only `customer_activities` table
(`0036_customer_activities.sql`), call/email/sms/whatsapp/meeting/
site_visit/note/other, listed and logged from a new panel on the
customer page.

## Phase 6 — Payments and Transfers
Nests the payments delivery sequence:

- **Phase A** — cash and Lipa Pole Pole sale modes, Plot Loan Accounts,
  repayment schedules, manual payment capture/allocation, approvals/
  reversals, receipts/statements.
- **Phase B** — arrears ageing, notifications/work queues, customer 360°
  financial view, dashboards, report library.
- **Phase C** — restructures/waivers/holidays, cancellations/repossessions/
  reallocations, agent commissions, title-transfer readiness workflows.
- **Phase D** — mobile-money/banking integrations, automated matching/
  receipting/reconciliation, customer self-service portal.

**Status (corrected 2026-09-26): Phases A and C essentially complete,
most of B too — only Phase D (real payment-provider integrations)
remains untouched.** A Plot Loan Account is created when a Lipa Pole
Pole sale is reserved (10% deposit / 12 instalments, a real generated
`repayment_schedule_entries` calendar — `0022_repayment_schedule.sql`),
with manual payment capture/allocation (a configurable penalty →
interest → principal waterfall — `finance_policy`), manual
interest/penalty charges and waivers, entry reversal, per-payment
receipts (`0031_payment_receipts.sql`) and a full running ledger
statement. **Phase C, all four items now built**: sale
cancellation/repossession + plot reallocation
(`0030_sale_lifecycle.sql`), loan restructures and repayment holidays
(`0033_loan_restructuring.sql`), agent commissions (see Phase 5 above),
and title-record tracking (`routes/title_records.rs`) as the
transfer-readiness piece. **Phase B, now fully built (2026-09-27/28)**:
arrears ageing (`days_in_arrears`, a 7-day grace period, on every loan
account and the dashboard's non-performing-loans aggregate), the
Executive dashboard and a report library (Sales, Inventory, Agent
Performance), a work queue aggregating arrears/expiring quotations/
pending approvals/lead follow-ups into one dashboard panel
(`routes/work_queue.rs`, `domain::work_queue`), and a customer-360
financial rollup (`routes/customers.rs::get_customer`). A
communications log — a separate item from Phase 5's note — is also now
built (2026-09-28). The Captured →
Verified → Posted payment
approval lifecycle from the original spec was never built — every
payment posts immediately; whether that's still wanted now that real
roles/permissions exist is an open product question, not a known gap.
**Phase D — mobile-money/banking integrations, automated matching/
receipting/reconciliation, the customer self-service portal — not
started at all.** This is the "integration" work explicitly deferred
per the 2026-09-26 decision to build the settings-driven
plug-in-configuration infrastructure first and the actual provider
wiring later.

## Phase 7 — Analytics and Integrations
Project-performance dashboards; plot-availability analytics; sales
conversion/agent performance; revenue/collection reports; GIS/satellite
mapping; accounting/payment/SMS/email/WhatsApp integrations.
**Status (corrected 2026-09-26): the analytics half is built** — the
Executive dashboard, Finance Overview, and the Sales/Inventory/Agent
Performance report library between them cover project performance,
plot availability, sales conversion, agent performance, and revenue/
collection. **The integrations half is entirely unbuilt**: no GIS/
satellite mapping, no accounting/SMS/email/WhatsApp integration of any
kind. See the settings-driven integration infrastructure below — the
deliberate next step before any of these get real provider code.

## Platform billing (parallel to the phases above)

SaaS subscription billing ([16](16-billing-and-subscriptions.md)) is an
operational concern for running the platform as a business, not a phase
in the product roadmap above — it can and should move independently.
**Status**: schema (`subscription_plans`, `organization_subscriptions`,
`billing_invoices`, `billing_webhook_events`) and a working, signature-
verified Paystack webhook receiver exist
(`crates/backend/src/paystack.rs`). **Corrected 2026-09-28 — this
section's own earlier claim was wrong about two of its three items:**
the org sign-up flow (`POST /api/v1/auth/signup`, `pages/signup.rs`)
and enforcement of subscription/trial status against feature access
(`tenant_gate::check`, run on login and on every authenticated
request) were already both built. Genuinely not built: a real
plan-selection UI backed by `subscription_plans` (pricing tiers a
signing-up org can actually choose) — today's signup form only takes a
free-text "preferred package" string. Deliberately not built yet
either: actually wiring a chosen plan to a live Paystack subscription,
since that's checkout/payment integration, in the same deferred bucket
as the rest of Phase 6D/7's provider work.

## UI/UX consistency — number/currency display, commission override clarity (2026-09-26)

Raised via user feedback (screenshots of the loan account detail page
and the project header's commission override control): repeating the
currency code beside every figure in a tight grid wastes horizontal
space and risks numbers wrapping mid-value, and "Override" as a bare
button label doesn't say what's being overridden.

**Done:**
- `format::format_amount` (bare number, no currency code) is now the
  default for every stat-card-style grid and dt/dd summary panel:
  dashboard (already the reference implementation this pattern was
  generalized from), Finance Overview (already correct), loan account
  detail's stat grid, the Sales report's stat-card row, and project
  detail's plot tiles + commercial-position panel. Each of those
  sections now shows the currency exactly once via a shared
  `.currency-note` badge (renamed from the dashboard-only
  `.dashboard-currency` it started as) instead of repeating it per
  figure.
- `.stat-card .stat-value` uses `clamp()` for responsive font-sizing
  and keeps `white-space: nowrap` (already present) so a figure never
  breaks mid-number, only ever shrinks or (via nowrap) stays intact.
- Left `format_money` (currency-inline) deliberately in two kinds of
  place: (1) tables/lists where each row is scanned independently
  (payment history, ledger entries, report rows, receipts' own line
  items) — no redundancy to remove there, each row is its own
  context; (2) printable/exportable documents (the payment receipt,
  the loan statement) — these are meant to be viewed or shared
  outside the surrounding app chrome (printed, screenshotted), where
  restating the currency per figure is the more correct convention,
  not a redundancy.
- `ProjectCommissionEditor` (project detail page) rewritten: the bare
  "Override" button is now "Override Commission Rate" (or "Edit
  Override" once one is active); the summary line reads "Default
  Commission: X% → Override Commission Rate: Y%" instead of a bare
  percentage with no reference point; a new optional
  `commission_rate_override_reason` (migration `0034`, stored on
  `projects`, cleared automatically whenever the override itself is
  cleared) is shown under the summary and captured in the edit form
  alongside a static "Applies to: this project" line (the only scope
  that exists today — no per-sale/per-agent override); "Clear
  override" is now "Remove Override / Restore Default".

**Deliberately not swept this pass** (isolated single/double money
mentions on a page, not a repeated grid — the "where appropriate"
carve-out the request itself named): customer detail, quotation
detail/list, approvals list, recent activity, the org-wide loan
accounts list. A future pass could still apply `.currency-note` there
if a page grows more than one or two money mentions.

## Bug fixes found while auditing (2026-09-26)

Two features were fully built on the backend, with working client
methods at every frontend API layer, but never actually reachable from
any page — found by working through cargo's small set of dead-code
warnings instead of assuming they were inert placeholders.

- **Tenant approval workflow**: a new organization signs up into
  `pending_approval` and needs the Platform Owner to approve or reject
  it before it can trial (`routes/platform.rs`, already correct). No
  page ever called `approve_organization`/`reject_organization`, and
  the org list/detail pages collapsed every status except
  `"deactivated"` into a green "Active" badge — `pending_approval`,
  `rejected`, `suspended`, and `terminated` all looked identical to a
  paying tenant, with no way to act on any of them. Fixed: a real
  `organization_status_meta` mapping (`frontend/src/format.rs`)
  covering all eleven statuses, and Approve/Reject controls on the
  org detail page (rejection requires a reason; approved-by/rejected-
  reason context is now shown once set). No org in production had
  actually hit this, but the next real signup would have been stuck
  forever with no way to unstick it.
- **Map feature unlinking**: linking a plot to a drawn map shape
  worked, but undoing it didn't — clicking a linked shape in edit mode
  deleted it outright (losing the boundary), and the dedicated unlink
  endpoint that preserves the shape for relinking to a different plot
  was never called from anywhere. Fixed: edit-mode clicks on a linked
  shape now unlink it server-side instead of deleting it locally;
  unlinked (draft) shapes still discard locally as before, since
  nothing was saved for them yet.

## Known bug: migrations can't run against a genuinely empty database (found 2026-10-01)

Found while testing the `app_user` RLS role change against a brand-new
local Postgres database rather than an already-seeded one — every
database this project has ever migrated (dev or production) already
had at least one `payments` row by the time
`database/migrations/0031_payment_receipts.sql` ran, so this was never
exercised before. That migration's backfill ends with
`select setval('payment_receipt_number_seq', (select count(*) from
payments), true);` — on an empty `payments` table `count(*)` is `0`,
and Postgres sequences reject `setval(seq, 0)` by default (`minvalue`
is `1`), so the whole migration run aborts. **Can't be fixed in place**:
0031 already succeeded in production, and `sqlx::migrate!` tracks
applied migrations by a checksum of the file's contents — editing it
would mismatch that checksum and break the *next* deploy everywhere
it's already applied, a far worse outcome than leaving the bug as
documented, known technical debt. A real fix needs either a deliberate
migration-history squash at a major version boundary, or a one-time
manual `setval` before first boot on any environment starting from
true scratch (a disaster-recovery restore, a new local dev clone,
etc.) — not something to improvise mid-task. No impact on any
already-running environment (dev or production), since both already
have payment rows.

## Integration infrastructure — settings-driven, before any provider (2026-09-26)

Decision: every remaining "integration" item across the phases above
(Phase 6D's mobile-money/banking + reconciliation + self-service
portal, Phase 7's GIS/satellite mapping and SMS/email/WhatsApp/
accounting) is explicitly deferred — not because it isn't wanted, but
because building six one-off provider integrations before there's a
settings surface to configure any of them means redoing that surface
six times. The infrastructure ships first: a generic, per-organization
integration-configuration system (provider type, credentials/endpoint,
enabled toggle, all editable from Settings, none of it hardcoded) that
real provider code can plug into later without another schema/UI
detour. This doc will be updated with what actually gets built as that
work lands, the same way this section itself was added rather than
silently starting work with no record of the decision.

## Sequencing principle

Manual interactive map creation and manual payment capture ship first, as
reliable operational systems; AI-assisted plan conversion and payment-
integration automation layer on afterward, once the manual path is proven
and there's real data to validate the automation against.
