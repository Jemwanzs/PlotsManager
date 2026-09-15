-- Reference data (not a repo "seed" — those are dev-only demo data; this
-- is real, needed in every environment including production), so it
-- belongs in a migration: the plan a newly signed-up tenant's 48-hour
-- trial (crates/backend/src/routes/auth.rs's `signup` handler) attaches
-- to before they ever pick a paid plan. `paystack_plan_code` is a
-- placeholder, not a real Paystack plan — nothing here is ever charged;
-- signup creates the trial without going through Paystack at all.
insert into subscription_plans (code, name, price, currency, billing_interval, paystack_plan_code, is_active)
values ('TRIAL', 'Free Trial', 0, 'KES', 'monthly', 'PLN_TRIAL_NOT_BILLED', true)
on conflict (code) do nothing;
