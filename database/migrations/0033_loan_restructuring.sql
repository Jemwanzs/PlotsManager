-- Audit trail for two loan-account interventions layered on top of the
-- existing schedule/ledger machinery. Neither touches
-- `plot_loan_accounts.outstanding_balance`/`amount_paid` or the
-- transaction ledger — no money moves and nothing owed is forgiven —
-- only the forward-looking `repayment_schedule_entries` calendar that
-- `loan_account_schedule_summary` reads to derive "days in arrears"/
-- "next instalment due" (0022_repayment_schedule.sql).
--
--   Repayment holiday: push every not-yet-fully-paid schedule entry's
--   due_date forward by N days, so a customer isn't marked overdue
--   during an agreed pause. Arrears already accrued before the
--   holiday are untouched — a holiday excuses the future, not the
--   past.
--
--   Restructure: replace the not-yet-fully-paid tail of the schedule
--   with a freshly amortized one at a new instalment amount and/or
--   frequency, covering the same remaining principal. Total owed
--   doesn't change, only how it's spread out going forward.
create table loan_repayment_holidays (
    id uuid primary key default gen_random_uuid(),
    loan_account_id uuid not null references plot_loan_accounts(id),
    organization_id uuid not null references organizations(id),
    holiday_days int not null check (holiday_days > 0),
    reason text,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now()
);
create index loan_repayment_holidays_loan_account_idx on loan_repayment_holidays(loan_account_id);

alter table loan_repayment_holidays enable row level security;
create policy loan_repayment_holidays_org_select on loan_repayment_holidays
    for select using (organization_id = public.current_org_id());
create policy loan_repayment_holidays_org_insert on loan_repayment_holidays
    for insert with check (organization_id = public.current_org_id());

create table loan_restructures (
    id uuid primary key default gen_random_uuid(),
    loan_account_id uuid not null references plot_loan_accounts(id),
    organization_id uuid not null references organizations(id),
    old_instalment_amount numeric(16, 2) not null,
    new_instalment_amount numeric(16, 2) not null,
    old_repayment_frequency_days int not null,
    new_repayment_frequency_days int not null,
    reason text,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now()
);
create index loan_restructures_loan_account_idx on loan_restructures(loan_account_id);

alter table loan_restructures enable row level security;
create policy loan_restructures_org_select on loan_restructures
    for select using (organization_id = public.current_org_id());
create policy loan_restructures_org_insert on loan_restructures
    for insert with check (organization_id = public.current_org_id());

update roles set permissions = permissions || '["finance:restructure"]'::jsonb
where not (permissions @> '["*"]'::jsonb) and not (permissions @> '["finance:restructure"]'::jsonb);
