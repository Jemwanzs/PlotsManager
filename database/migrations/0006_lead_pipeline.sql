-- Leads/prospects (docs/07 §"Sales funnel") were only ever specified as
-- a funnel-stage name, never as a distinct entity — the legacy VBA
-- system had no leads table either (docs/02), and `customers` already
-- captures a walk-in with just a name and enriches it later (see the
-- doc comment on domain::CreateCustomerInput). So a lead is a
-- `customers` row before it has a sale, not a separate table: adding a
-- pipeline stage here instead of standing up a parallel leads/
-- conversion system nothing actually calls for. "Converted" isn't a
-- stage value — it's derived from a `plot_sales` row existing, so it
-- can't drift out of sync with the real sale state.
alter table customers add column stage text not null default 'new'
    check (stage in ('new', 'contacted', 'site_visit', 'negotiating', 'lost'));
alter table customers add column source text;
alter table customers add column next_follow_up_at date;
alter table customers add column notes text;
