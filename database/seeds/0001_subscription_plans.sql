-- Static reference data, safe to run repeatedly. Run manually against a
-- fresh database after migrations — not applied automatically by the
-- backend (`sqlx::migrate!` only runs database/migrations/, never
-- database/seeds/), since seeding is an operator decision, not a boot
-- step. The paystack_plan_code values are placeholders — replace with the
-- real plan codes created in the Paystack dashboard before going live.
insert into subscription_plans (code, name, price, currency, billing_interval, paystack_plan_code)
values
    ('starter_monthly', 'Starter', 4999, 'KES', 'monthly', 'PLN_starter_monthly_placeholder'),
    ('growth_monthly', 'Growth', 14999, 'KES', 'monthly', 'PLN_growth_monthly_placeholder'),
    ('starter_annual', 'Starter (Annual)', 49990, 'KES', 'annual', 'PLN_starter_annual_placeholder'),
    ('growth_annual', 'Growth (Annual)', 149990, 'KES', 'annual', 'PLN_growth_annual_placeholder')
on conflict (code) do nothing;
