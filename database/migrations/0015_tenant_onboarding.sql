-- Tenant onboarding: public sign-up no longer grants automatic access.
-- A new registration lands in `pending_approval`; only the Platform
-- Owner approving it starts the trial. Existing organizations are
-- untouched (their current `status` value, typically 'active', stays
-- exactly as it is) — this only changes what happens for *new*
-- sign-ups from here on.

alter table organizations drop constraint organizations_status_check;
alter table organizations add constraint organizations_status_check
    check (status in (
        'active', 'deactivated',
        'pending_approval', 'trial_active', 'subscription_active',
        'trial_expired', 'payment_due', 'suspended',
        'termination_requested', 'terminated', 'rejected'
    ));
alter table organizations alter column status set default 'pending_approval';

alter table organizations
    add column business_registration_number text,
    add column sector text,
    add column business_location text,
    add column contact_person_name text,
    add column expected_users int,
    add column number_of_branches int,
    add column preferred_package_code text,
    add column approved_at timestamptz,
    add column approved_by uuid references users(id),
    add column rejected_at timestamptz,
    add column rejected_reason text;

-- Every existing organization predates this whole concept of an
-- "application" — backfilled as already approved (by nobody in
-- particular) rather than leaving these columns misleadingly null,
-- which would make a later "was this ever approved?" query wrong for
-- every tenant that existed before this migration.
update organizations set approved_at = created_at where status not in ('pending_approval', 'rejected');

create table terms_versions (
    id uuid primary key default gen_random_uuid(),
    version_label text not null unique,
    body text not null,
    is_current boolean not null default false,
    created_at timestamptz not null default now()
);
-- Only one version is ever "current" at a time — the one shown to a
-- new sign-up and the one new acceptances are checked against.
create unique index terms_versions_one_current_idx on terms_versions (is_current) where is_current;

create table terms_acceptances (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    user_id uuid not null references users(id),
    terms_version_id uuid not null references terms_versions(id),
    accepted_at timestamptz not null default now(),
    ip_address text,
    user_agent text
);
create index terms_acceptances_org_idx on terms_acceptances(organization_id);

insert into terms_versions (version_label, body, is_current) values (
    '1.0',
    'These Terms & Conditions govern access to and use of Real Estate Manager ("the Platform"). By creating an organization account you agree to: (1) provide accurate registration information; (2) use the Platform only for lawful property/plot sales management; (3) keep your account credentials confidential; (4) accept that your workspace is subject to Platform Owner review and activation before use; (5) accept the subscription and billing terms presented at the time of your chosen package; (6) allow the Platform to preserve your organization''s operational data according to its retention rules even if your subscription is suspended or terminated. The Platform Owner may update these Terms from time to time; material changes will require re-acceptance of a new version.',
    true
);

alter table terms_versions enable row level security;
alter table terms_acceptances enable row level security;
create policy terms_versions_public_select on terms_versions for select using (true);
create policy terms_acceptances_org_select on terms_acceptances for select
    using (organization_id = public.current_org_id());
