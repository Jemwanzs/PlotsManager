-- Legacy data migration readiness (Prime Plots Property gap analysis):
-- `plot_sales` has always had a single `plot_id` and a single
-- `customer_id` — real legacy records break both assumptions. The
-- loan registry has one loan/account spanning several plots (e.g. one
-- 4,400,000 loan across PL.7,8,9,10), and the customer register has
-- joint buyers recorded as one row with a concatenated name (e.g.
-- "CATHERINE A OHOLA/ELIZABETH A OHOLA").
--
-- These two tables are additive, not a replacement: `plot_sales.
-- plot_id`/`customer_id` keep meaning exactly what they always have —
-- the primary plot and primary buyer — so every existing query,
-- report, and finance calculation that reads those columns directly
-- keeps working unchanged. The loan/finance side of a sale genuinely
-- is one account for the whole transaction (one EMI, one outstanding
-- balance covering every plot on it), so `plot_loan_accounts.sale_id`
-- doesn't need to change either — only *which plots and customers*
-- attach to that one sale needed a home.
--
-- Every `plot_sales` row (existing and future) gets a matching
-- "primary" row in both tables at creation time, so these are always
-- the complete list — code that wants "every plot/customer on this
-- sale" reads these, not the old columns plus these combined.
create table sale_plots (
    id uuid primary key default gen_random_uuid(),
    sale_id uuid not null references plot_sales(id),
    plot_id uuid not null references plots(id),
    unique (sale_id, plot_id)
);
-- A plot can only ever be on one sale, same as the existing
-- `plot_sales_one_active_per_plot` — this is that same guarantee
-- extended to non-primary plots.
create unique index sale_plots_plot_uidx on sale_plots(plot_id);

create table sale_customers (
    id uuid primary key default gen_random_uuid(),
    sale_id uuid not null references plot_sales(id),
    customer_id uuid not null references customers(id),
    role text not null default 'primary'
        check (role in ('primary', 'joint', 'representative')),
    unique (sale_id, customer_id)
);

insert into sale_plots (sale_id, plot_id)
select id, plot_id from plot_sales;

insert into sale_customers (sale_id, customer_id, role)
select id, customer_id, 'primary' from plot_sales;

alter table sale_plots enable row level security;
alter table sale_customers enable row level security;

create policy sale_plots_org_select on sale_plots for select
    using (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));

create policy sale_customers_org_select on sale_customers for select
    using (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));
