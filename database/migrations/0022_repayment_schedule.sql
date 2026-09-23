-- Populates `repayment_schedule_entries` (created in 0001, never
-- written to until now) for every existing Lipa Pole Pole loan
-- account, and adds three views that turn that static plan plus the
-- account's own `amount_paid` into a live "what's overdue right now"
-- read — no scheduler needed, since nothing here is stored mutable
-- state that could go stale between payments. Application code
-- (routes/sales.rs) generates the same rows for every new loan
-- account going forward.
--
-- The plan itself is exactly what routes/sales.rs::execute_sale
-- already computes at sale time: a deposit (instalment 0) plus 12
-- equal instalments, `repayment_frequency_days` apart starting at
-- `start_date`. Interest is layered on separately via manual charges
-- (see 0020_loan_ledger.sql), not amortized into this schedule, so
-- every row's `interest_due` stays 0.

insert into repayment_schedule_entries
    (loan_account_id, instalment_number, due_date, principal_due, interest_due, fees_due, total_due, amount_paid, status)
select
    pla.id,
    0,
    pla.start_date,
    pla.deposit_required,
    0,
    0,
    pla.deposit_required,
    0,
    'upcoming'
from plot_loan_accounts pla
where pla.deposit_required > 0
on conflict (loan_account_id, instalment_number) do nothing;

insert into repayment_schedule_entries
    (loan_account_id, instalment_number, due_date, principal_due, interest_due, fees_due, total_due, amount_paid, status)
select
    pla.id,
    gs.n,
    pla.start_date + (pla.repayment_frequency_days * gs.n) * interval '1 day',
    pla.instalment_amount,
    0,
    0,
    pla.instalment_amount,
    0,
    'upcoming'
from plot_loan_accounts pla
cross join generate_series(1, 12) as gs(n)
where pla.instalment_amount > 0
on conflict (loan_account_id, instalment_number) do nothing;

-- Per-entry: how much of `total_due` has actually landed, given the
-- account's `amount_paid` applied against entries oldest-due-first
-- (a payment doesn't know which instalment it's "for" — the account
-- only tracks one running total — so this allocates it the same way
-- a customer paying down the oldest thing first would expect).
create or replace view loan_schedule_entry_paid as
select
    e.id,
    e.loan_account_id,
    e.instalment_number,
    e.due_date,
    e.total_due,
    greatest(0::numeric, least(e.total_due,
        pla.amount_paid - coalesce(sum(e.total_due) over (
            partition by e.loan_account_id
            order by e.due_date, e.instalment_number
            rows between unbounded preceding and 1 preceding
        ), 0)
    )) as paid_amount
from repayment_schedule_entries e
join plot_loan_accounts pla on pla.id = e.loan_account_id;

-- Adds the derived status a stored column can't keep fresh on its
-- own: `due_date > current_date` (upcoming) still holds even if
-- nothing about the account changes today, and stops holding the
-- instant the calendar crosses it — a live view recomputes that for
-- free on every read, a stored flag would need a job to ever flip.
-- 7-day grace period: a literal constant here, same as the ledger's
-- waterfall order, until a real "make this configurable" phase.
--
-- `paid_amount > 0` is deliberately NOT checked before the due-date
-- comparisons: a partially-paid instalment that's still short past
-- its grace period is still overdue — checking payment-completeness
-- first would let a partial payment permanently mask real arrears.
create or replace view loan_schedule_entry_status as
select
    *,
    case
        when paid_amount >= total_due then 'paid'
        when due_date > current_date then 'upcoming'
        when due_date > current_date - 7 then 'due'
        else 'overdue'
    end as instalment_status
from loan_schedule_entry_paid;

-- One row per loan account: the oldest still-overdue due date (drives
-- `days_in_arrears`) and the next not-yet-fully-paid instalment
-- (drives "Next instalment" / "Next due date").
create or replace view loan_account_schedule_summary as
select
    loan_account_id,
    min(due_date) filter (where instalment_status = 'overdue') as oldest_overdue_due_date,
    min(due_date) filter (where paid_amount < total_due) as next_instalment_due_date,
    (array_agg(total_due order by due_date, instalment_number) filter (where paid_amount < total_due))[1] as next_instalment_amount
from loan_schedule_entry_status
group by loan_account_id;
