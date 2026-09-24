-- Legacy data migration readiness (Prime Plots Property gap analysis):
-- the customer register the first tenant is migrating from carries a
-- full KYC set this table never had a home for — title, KRA PIN,
-- postal/physical address, city, a legacy customer number (PPP_C001-
-- style), customer type (individual/company/joint), and full next-of-
-- kin details. All nullable: `CreateCustomerInput` stays deliberately
-- minimal (see its own doc comment — capture a lead now, enrich
-- later), these are the fields that later enrichment now has
-- somewhere to go.
alter table customers
    add column title text,
    add column customer_type text not null default 'individual'
        check (customer_type in ('individual', 'company', 'joint')),
    add column kra_pin text,
    add column postal_address text,
    add column city text,
    add column physical_address text,
    add column legacy_customer_number text,
    add column next_of_kin_name text,
    add column next_of_kin_relationship text,
    add column next_of_kin_mobile text,
    add column next_of_kin_id_number text,
    add column next_of_kin_address text;

-- A legacy customer number is only meaningful (and only needs to be
-- unique) within the organization it was migrated into — not globally,
-- and never required, since most customers won't have one.
create unique index customers_legacy_number_uidx
    on customers (organization_id, legacy_customer_number)
    where legacy_customer_number is not null;

-- Backfills the new customers:edit permission onto every existing
-- role — same safety net every permission-enforcement migration this
-- session has used. Editing a customer's profile didn't exist as a
-- route at all before this, so there's nothing behavioural changing
-- for anyone yet.
update roles
set permissions = permissions || '["customers:edit"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["customers:edit"]'::jsonb);
