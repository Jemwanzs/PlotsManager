-- Provisions the least-privilege `app_user` role that
-- docs/10-database-and-security-design.md has flagged as an open task
-- since the RLS policies were first written: the backend's only
-- Postgres connection today uses a BYPASSRLS-capable role for
-- everything (migrations, webhooks, and ordinary requests alike), so
-- the RLS policies on every tenant table exist in schema but enforce
-- nothing. `app_user` is NOLOGIN — the backend never authenticates a
-- new connection as it, it switches its *existing* connection to it
-- per-transaction via `set local role app_user` (see
-- crates/backend/src/extractors.rs::TenantTx), which reverts
-- automatically at commit/rollback. No new credentials, no new
-- Railway secret, no second connection pool.
do $$
begin
  if not exists (select 1 from pg_roles where rolname = 'app_user') then
    create role app_user nologin nobypassrls;
  end if;
end
$$;

grant app_user to current_user;

-- select/insert/update only, matching the backend's own posture today
-- (no route deletes a posted financial row — see this doc's "No
-- deletion of posted financial transactions"). Granted across every
-- RLS-enabled table as of this migration, not just the two route
-- files (customers.rs, sales.rs) switched over in this same change —
-- so each later file's own migration to TenantTx is app code only,
-- not another grant statement.
-- `project_map_versions` (from 0001) was dropped and replaced by
-- `project_maps` in 0009_project_map.sql — granting on a table that no
-- longer exists fails the whole migration, caught by testing this
-- against a genuinely empty database rather than one of the long-lived
-- dev/production databases that already had 0009 applied before this
-- migration was ever written.
grant select, insert, update on
  organizations, branches, roles, users, role_assignments, projects,
  project_maps, plot_status_config, plots,
  customers, customer_activities, plot_sales, sale_customers, sale_plots,
  plot_loan_accounts, repayment_schedule_entries, payments,
  loan_ledger_entries, loan_restructures, loan_repayment_holidays,
  agent_commissions, quotations, approval_requests, title_records,
  documents, audit_log, subscription_plans, organization_subscriptions,
  billing_invoices, billing_webhook_events, integration_configs,
  terms_versions, terms_acceptances, migration_batches,
  migration_staging_rows
  to app_user;

-- Table-level grants aren't enough on their own: Postgres RLS denies
-- every row for a command with no matching policy, even to a role with
-- the table-level privilege. `customers.rs`/`sales.rs` (the two files
-- switched to `TenantTx` in this change) only had SELECT policies on
-- several tables they write to — caught by actually running an insert
-- under `app_user` against a seeded local database, not by reasoning
-- about it. Every `using`/`with check` here matches its table's
-- existing `_org_select` policy's own org-scoping expression exactly
-- (some direct, some via a join where the table has no organization_id
-- column of its own) — see `database/migrations/0001_init.sql`,
-- `0008_approvals.sql`, `0026_sale_plots_and_customers.sql` for each
-- one's original SELECT policy. Only the write shapes these two files
-- actually use are added — a delete policy, for instance, isn't, since
-- neither file ever deletes a row.
create policy customers_org_insert on customers for insert
    with check (organization_id = public.current_org_id());
create policy customers_org_update on customers for update
    using (organization_id = public.current_org_id());

create policy plot_sales_org_insert on plot_sales for insert
    with check (organization_id = public.current_org_id());
create policy plot_sales_org_update on plot_sales for update
    using (organization_id = public.current_org_id());

create policy plots_org_update on plots for update
    using (project_id in (select id from projects where organization_id = public.current_org_id()));

create policy plot_loan_accounts_org_insert on plot_loan_accounts for insert
    with check (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));
create policy plot_loan_accounts_org_update on plot_loan_accounts for update
    using (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));

create policy repayment_schedule_org_insert on repayment_schedule_entries for insert
    with check (loan_account_id in (
        select pla.id from plot_loan_accounts pla
        join plot_sales ps on ps.id = pla.sale_id
        where ps.organization_id = public.current_org_id()
    ));

create policy sale_plots_org_insert on sale_plots for insert
    with check (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));
create policy sale_plots_org_update on sale_plots for update
    using (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));

create policy sale_customers_org_insert on sale_customers for insert
    with check (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));

create policy approval_requests_org_update on approval_requests for update
    using (organization_id = public.current_org_id());
