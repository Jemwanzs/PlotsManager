# 11 — Reports and Analytics

Dashboards lead with charts and status indicators, backed by exact figures
and drill-down tables — not raw tables as the primary view. Every widget
supports filters (date, project, branch, location, agent, payment mode,
account status), and clicking a figure opens the underlying records,
subject to permission.

## Dashboards

**Executive**: total projects/plots; availability breakdown (available,
reserved, sold, fully paid, transferred); total portfolio sales value;
billed/collected/outstanding; cash vs. Lipa Pole Pole mix; principal vs.
interest income; collection rate; arrears/portfolio-at-risk; expected vs.
actual collections; sales/collection trends; top projects/branches/agents;
inventory absorption rate; forecast collections and cash flow.

**Project**: interactive colour-coded plot map; status composition; sales
value/collections by project; average price per plot/unit area; discount
analysis; cash/instalment mix; repayment performance; arrears by project;
inventory forecast; sales velocity / estimated sell-out period.

**Finance and collections**: collections today/week/month/year; due vs.
collected; current/overdue balances; arrears ageing; accounts needing
follow-up; unverified/rejected/reversed payments; suspense/unallocated
receipts; interest/penalties/fees/waivers; early settlements/overpayments;
collection performance by officer; expected collection calendar.

**Sales**: leads/reservations/bookings/completed sales; conversion by
stage; sales by agent/team/branch/project; cash vs. instalment; average
price/discount; high-demand plot categories; commissions
earned/approved/paid; cancellation/reallocation trends.

**Customer-level**: total exposure; repayment consistency; missed/late
instalments; promise-to-pay performance; account status/risk flag;
project/plot portfolio; estimated completion date.

## Report library

- **Sales and inventory**: plot register; status breakdown; sales by
  project/branch/region/agent; cash vs. instalment; pricing/discount
  report; cancelled/repossessed/reallocated plots; status history.
- **Financial and repayment**: account balances; repayment register/
  schedules; expected vs. actual collections; daily collections; payment
  method/collection-account report; principal/interest/fees/penalties;
  overpayment/credit report; unverified/suspense payments; reversals/
  waivers/adjustments; full-settlement report.
- **Arrears and portfolio**: ageing report; days-past-due; delinquent
  customers; portfolio-at-risk by configurable threshold; officer
  collection portfolio; promise-to-pay; restructured accounts; default/
  cancellation/repossession report.
- **Management and audit**: project profitability/sales performance;
  agent productivity/conversion; collection officer performance; approval
  turnaround time; user activity/audit; price-change/discount-approval
  report; payment reversals/exceptions; title-transfer readiness.

## Report experience

Saved filters/views; date/period comparison; grouping/sorting/drill-down;
organisation branding; prepared-by/generated-at metadata; page
numbers/print-friendly headers; PDF/Excel/CSV export; role-based masking
of sensitive customer/financial fields; scheduled email distribution
(later phase).

## Legacy reality (see [02](02-existing-vba-system-analysis.md))

Cross-checked against the actual legacy workbook (`#DailyReportPDF/`,
`#ExtractsPDF/`, `#PaymentSchedulesPDF/`, `#Extracts(Reports)/`):

- The Executive Dashboard KPI set above matches what the legacy dashboard
  already shows almost exactly: total customers/projects/plots, total
  loans, active loans, active loan book, and **Performing vs.
  Non-Performing** counts/amounts (filterable by year or "since
  inception"). Performing/Non-Performing is the legacy system's only
  arrears bucket — a single binary split, not the ageing tiers specified
  above — which confirms the ageing-bucket design is a real upgrade, not
  invention.
- **Add a Daily Activity Report** to the report library: a per-staff,
  per-day productivity log (site visits, savings made, sales made, new
  business leads, office visits, walk-ins, instalments made, transfers).
  This exists and is used in the legacy system today but isn't covered
  anywhere above — it's a lightweight CRM/activity report, distinct from
  the financial reports.
- One open product question the legacy system raises but doesn't answer
  for us: it also has a **customer feedback/ratings module** (comment
  type, free text, gender, 1–5 star rating) that nothing in this doc set
  covers. Needs an explicit in/out decision rather than being silently
  dropped.
