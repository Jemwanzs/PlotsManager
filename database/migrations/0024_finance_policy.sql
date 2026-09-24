-- Three previously-hardcoded finance behaviors, made per-organization
-- configurable: the payment allocation waterfall order (was a literal
-- constant in routes/loan_accounts.rs::allocate_waterfall), the
-- overdue grace period (was hardcoded `7` in 0022_repayment_schedule
-- .sql's views), and a suggested rate for manual interest/penalty
-- charges (routes/loan_accounts.rs::post_charge has no automatic
-- charging to drive yet — no scheduler exists — this only powers a
-- "use policy rate" suggestion on that form).
alter table organizations
    add column allocation_order text[] not null default array['penalty', 'interest', 'principal'],
    add column finance_grace_period_days int not null default 7,
    add column interest_enabled boolean not null default false,
    add column interest_rate_type text not null default 'percentage',
    add column interest_rate_value numeric(8, 4) not null default 0,
    add column penalty_enabled boolean not null default false,
    add column penalty_rate_type text not null default 'percentage',
    add column penalty_rate_value numeric(8, 4) not null default 0;

-- Re-point the schedule-status view at the organization's own grace
-- period instead of the hardcoded `7` days it shipped with.
create or replace view loan_schedule_entry_status as
select
    p.*,
    case
        when p.paid_amount >= p.total_due then 'paid'
        when p.due_date > current_date then 'upcoming'
        when p.due_date > current_date - o.finance_grace_period_days then 'due'
        else 'overdue'
    end as instalment_status
from loan_schedule_entry_paid p
join plot_loan_accounts pla on pla.id = p.loan_account_id
join plot_sales ps on ps.id = pla.sale_id
join organizations o on o.id = ps.organization_id;
