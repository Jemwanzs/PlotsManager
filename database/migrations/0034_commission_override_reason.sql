-- Lets an org-default commission override carry the "why" alongside
-- the rate itself, matching every other sensitive-override action in
-- this app (sale cancellation, repossession, loan restructuring) that
-- already stores a reason. Cleared alongside the rate whenever the
-- override itself is cleared (routes/projects.rs::update_project_commission),
-- so a stale reason can never survive past the override it explained.
alter table projects
    add column commission_rate_override_reason text;
