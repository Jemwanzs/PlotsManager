-- The transaction ledger behind "Finance -> Receivable / Lipa Pole
-- Pole Account -> View Statement" — every charge, payment, waiver and
-- reversal against a loan account, append-only, in the order they
-- happened. This is the missing piece the rest of the finance
-- enhancement (payment allocation, statements, interest/penalty
-- configuration, reversals) all build on:
--
--   Plot -> Sale/Agreement (plot_sales) -> Receivable Account
--   (plot_loan_accounts) -> Transaction Ledger (this table) -> Statement
--
-- `payments` (0001_init.sql) is kept exactly as-is and keeps being
-- written to by record_payment — nothing that already reads it
-- (Payment History on the loan account detail page) breaks. Each
-- `payment` ledger entry links back to its originating `payments` row
-- via `reference_payment_id` rather than replacing that table; the
-- ledger adds the allocation breakdown and running balance `payments`
-- never had.
create table loan_ledger_entries (
    id uuid primary key default gen_random_uuid(),
    loan_account_id uuid not null references plot_loan_accounts(id),
    organization_id uuid not null references organizations(id),
    entry_type text not null check (entry_type in (
        'payment', 'charge_interest', 'charge_penalty',
        'waiver_interest', 'waiver_penalty', 'reversal', 'adjustment'
    )),
    entry_date date not null,
    -- Gross amount of this entry: a payment's received amount, or a
    -- charge's/waiver's amount. Always positive; direction is implied
    -- by entry_type, not sign.
    gross_amount numeric(16, 2) not null,
    -- Signed effect on each outstanding component. A payment's deltas
    -- are negative (they reduce what's owed); a charge's deltas are
    -- positive (they increase it); a waiver is negative on the
    -- component it forgives. principal_delta + interest_delta +
    -- penalty_delta always equals the entry's net effect on
    -- outstanding_balance.
    principal_delta numeric(16, 2) not null default 0,
    interest_delta numeric(16, 2) not null default 0,
    penalty_delta numeric(16, 2) not null default 0,
    -- Running outstanding balance immediately after this entry —
    -- snapshotted at write time so the statement renders straight
    -- from stored rows instead of re-summing the whole ledger on
    -- every read.
    balance_after numeric(16, 2) not null,
    method text,
    external_reference text,
    notes text,
    reference_payment_id uuid references payments(id),
    reversal_of_entry_id uuid references loan_ledger_entries(id),
    created_by uuid not null references users(id),
    created_at timestamptz not null default now()
);
create index loan_ledger_entries_loan_account_idx on loan_ledger_entries(loan_account_id, entry_date, created_at);
create index loan_ledger_entries_org_idx on loan_ledger_entries(organization_id);

alter table loan_ledger_entries enable row level security;
create policy loan_ledger_entries_org_select on loan_ledger_entries
    for select using (organization_id = public.current_org_id());

-- Backfills one ledger entry per existing payment row, so a plot that
-- already has payment history shows a real statement instead of
-- starting empty. Entirely principal (no interest/penalty charge has
-- ever been posted anywhere in this app yet, so every historical
-- payment can only have applied to principal) — running balances
-- recomputed in payment order per account.
insert into loan_ledger_entries
    (loan_account_id, organization_id, entry_type, entry_date, gross_amount,
     principal_delta, interest_delta, penalty_delta, balance_after,
     method, external_reference, reference_payment_id, created_by, created_at)
select
    p.loan_account_id,
    ps.organization_id,
    'payment',
    p.payment_date,
    p.amount,
    -p.amount,
    0,
    0,
    greatest(0, pla.principal - sum(p.amount) over (
        partition by p.loan_account_id order by p.payment_date, p.created_at
        rows between unbounded preceding and current row
    )),
    p.method,
    p.external_reference,
    p.id,
    p.captured_by,
    p.created_at
from payments p
join plot_loan_accounts pla on pla.id = p.loan_account_id
join plot_sales ps on ps.id = pla.sale_id
order by p.loan_account_id, p.payment_date, p.created_at;
