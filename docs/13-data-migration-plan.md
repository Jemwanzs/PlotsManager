# 13 — Data Migration Plan

Migrating real organisations off the Excel/VBA workbook and onto this
platform. The sheet/column structure is now documented in
[02](02-existing-vba-system-analysis.md); this plan itself — actually
running a migration — has not started.

## Legacy sheets to migrate

From `02`'s data model table: `CustomerInfo` → `customers` (+ KYC photo
files, currently local file-path references into a shared `#images/`
folder — these need to move to real object storage, not just have their
rows copied); `Projects` → `projects`; `Project_PLOTS` → `plots` (the
free-text "Plot Description" becomes the source for a real plot number,
deduplicated **per project** rather than globally); `LoanRegister` +
`LoanPayment`/`OtherCharges`/`PrintLoan` → `plot_sales` +
`plot_loan_accounts` + `payments` (the single flat `PrintLoan` ledger is
the most reliable source of truth for historical transactions since it's
what the legacy statements were generated from); `SuspenseAccount` →
informs the opening balance of a GL-style clearing account, not a
per-customer table; `AllRejected` → historical reference only, not
migrated into live data. `DailyReport`/`StaffReport` and `Comments` are
migrate-if-kept, pending the open product decision in
[02](02-existing-vba-system-analysis.md#11-open-question-for-the-user)
and [11](11-reports-and-analytics.md).

## Planned sequence

1. **Extract**: export each relevant sheet from the live workbook to
   CSV/structured form (projects, plots, customers, bookings, payments).
2. **Map**: define a field-by-field mapping from legacy columns to the new
   schema (`crates/api/migrations/0001_init.sql`), flagging anything with
   no clean equivalent (e.g. free-text statuses that need to collapse into
   the fixed `PlotStatus` set).
3. **Validate**: run duplicate/consistency checks before import — duplicate
   plot numbers, orphaned payments (no matching plot/customer), sales with
   no corresponding plot status, negative or inconsistent balances.
4. **Dry-run import**: load into a staging environment, reconcile totals
   (plot counts, sums of payments per project) against the legacy reports
   in `legacy-excel/PlotsManager/#Extracts(Reports)/` and
   `#PaymentSchedulesPDF/` before trusting the import.
5. **Reconcile financials**: every migrated Plot Loan Account's opening
   balance and repayment schedule must reconcile to the legacy payment
   history — this is the highest-risk part of the migration and needs
   sign-off before cutover, not just an automated check.
6. **Cutover**: freeze the legacy workbook for new entries, do the final
   import, and treat the workbook as historical read-only reference
   afterward.

## Data handling

All legacy source data is real customer/financial data. Migration tooling
and any exported intermediate files stay under `legacy-excel/` or another
gitignored path — never committed, even in a "sample" or "anonymised" form
unless it has been deliberately and verifiably scrubbed first.
