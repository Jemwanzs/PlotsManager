-- Sale cancellation / repossession / reallocation — the gap flagged in
-- `plot_sales_one_active_per_plot`'s own comment (0001_init.sql) and in
-- `pages/project_detail.rs::can_start_sale`'s doc comment: "cancellation,
-- restructure, etc. — not built yet". This is that workflow's schema.
--
-- `plot_sales` gets a real lifecycle instead of being implicitly
-- "forever active" the moment it's created. `status_reason`/
-- `status_changed_at`/`status_changed_by` are an audit trail, not just
-- a flag — matches this app's established preference for keeping WHY
-- alongside WHAT changed, not just the latest state.
alter table plot_sales
    add column status text not null default 'active'
        check (status in ('active', 'cancelled', 'repossessed')),
    add column status_reason text,
    add column status_changed_at timestamptz,
    add column status_changed_by uuid references users(id);

-- Denormalized copy of the same status onto `sale_plots` — needed so
-- `sale_plots_plot_uidx` below can be a partial index scoped to active
-- sales without a subquery (Postgres partial-index predicates can only
-- reference the indexed table's own columns). Kept in lockstep with
-- `plot_sales.status` by the cancel/repossess routes, in the same
-- transaction.
alter table sale_plots
    add column status text not null default 'active'
        check (status in ('active', 'cancelled', 'repossessed'));

-- Both of these were unconditional unique indexes — "a plot can only
-- ever have one plot_sales/sale_plots row, full stop" — which is
-- exactly what made cancellation impossible to build: a cancelled sale
-- would permanently block that plot from ever being resold. Scoping
-- both to `status = 'active'` is the exact fix `0001_init.sql`'s own
-- comment on `plot_sales_one_active_per_plot` anticipated: "Once
-- cancelled/repossessed sales are modelled with a status column,
-- replace this with a partial unique index over active statuses only,
-- so a cancelled sale can be superseded by a new one."
drop index plot_sales_one_active_per_plot;
create unique index plot_sales_one_active_per_plot on plot_sales(plot_id) where status = 'active';

drop index sale_plots_plot_uidx;
create unique index sale_plots_plot_uidx on sale_plots(plot_id) where status = 'active';

-- Backfills the new plots:transactions.cancel permission onto every
-- existing role — same safety net every permission-enforcement
-- migration this session has used. Cancel/repossess/reallocate didn't
-- exist as routes before this, so nothing behavioural changes for
-- anyone yet.
update roles
set permissions = permissions || '["plots:transactions.cancel"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["plots:transactions.cancel"]'::jsonb);
