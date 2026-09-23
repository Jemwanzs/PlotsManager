-- Backfills the new finance:post_charges permission onto every
-- existing role — same safety net every permission-enforcement
-- migration this session has used. Before this migration, posting a
-- manual interest/penalty charge didn't exist as a route at all, so
-- there's nothing behavioural changing for anyone yet; this just
-- keeps every pre-existing role ready the moment
-- routes/loan_accounts.rs::post_charge ships gated behind it, rather
-- than making every tenant's admin re-grant it by hand.
update roles
set permissions = permissions || '["finance:post_charges"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["finance:post_charges"]'::jsonb);
