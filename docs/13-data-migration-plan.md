# 13 — Data Migration Plan

Migrating real organisations off the Excel/VBA workbook and onto this
platform. Not started — depends on [02](02-existing-vba-system-analysis.md)
being done first, since the actual sheet/column structure isn't documented
yet.

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
