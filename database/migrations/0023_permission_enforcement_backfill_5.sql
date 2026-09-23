-- Backfills the new finance:reverse permission onto every existing
-- role — same safety net every permission-enforcement migration this
-- session has used. Before this migration, reversing a payment/charge
-- or waiving interest/penalty didn't exist as a route at all, so
-- there's nothing behavioural changing for anyone yet; this just
-- keeps every pre-existing role ready the moment
-- routes/loan_accounts.rs::reverse_entry/post_waiver ship gated
-- behind it, rather than making every tenant's admin re-grant it by
-- hand.
update roles
set permissions = permissions || '["finance:reverse"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["finance:reverse"]'::jsonb);
