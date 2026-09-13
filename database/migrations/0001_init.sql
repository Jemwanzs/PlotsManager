-- Phase 2 foundation schema. Platform-neutral Postgres, run against
-- Railway Postgres by the backend on boot (`sqlx::migrate!`, see
-- crates/backend/src/main.rs) — no external migration CLI required.
--
-- Architecture: the frontend never connects to this database directly.
-- Every request goes through the Rust backend (crates/backend), which is
-- the primary enforcement point for authentication, authorization, and
-- tenant isolation. Row-Level Security below is *defense in depth*, not
-- the primary boundary: the backend opens a transaction, runs
-- `select set_config('app.current_organization_id', $1, true)` with the
-- authenticated caller's organization before any tenant-scoped query, and
-- these policies check that session-local setting. A bug that forgets a
-- WHERE clause in application code still can't leak across tenants; a
-- bug that forgets to set the session variable gets zero rows back, not
-- someone else's data (current_setting(..., true) is null, and null
-- never equals an organization_id).

create extension if not exists "pgcrypto";
-- Case-insensitive email column (users.email) so "A@x.com" and "a@x.com"
-- collide at the database constraint level, not just in application code.
create extension if not exists "citext";

-- Read by every RLS policy below. No elevated privileges needed — it
-- only reads a session-local setting the backend sets per transaction.
create or replace function public.current_org_id()
returns uuid
language sql
stable
as $$
    select nullif(current_setting('app.current_organization_id', true), '')::uuid
$$;

create table organizations (
    id uuid primary key default gen_random_uuid(),
    name text not null,
    code text not null unique,
    currency text not null default 'KES',
    created_at timestamptz not null default now()
);

create table branches (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    name text not null,
    code text not null,
    region text,
    unique (organization_id, code)
);

create table roles (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    name text not null,
    permissions jsonb not null default '[]',
    unique (organization_id, name)
);

-- Authentication lives here, owned entirely by the backend (argon2
-- password hashing, JWT/session issuance — see crates/backend/src/auth).
-- No Auth-as-a-service dependency.
create table users (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    branch_id uuid references branches(id),
    full_name text not null,
    email citext not null unique,
    password_hash text not null,
    is_active boolean not null default true,
    created_at timestamptz not null default now()
);

create table role_assignments (
    id uuid primary key default gen_random_uuid(),
    user_id uuid not null references users(id),
    role_id uuid not null references roles(id),
    project_id uuid,
    branch_id uuid references branches(id)
);
-- A plain `unique (user_id, role_id, project_id)` constraint wouldn't
-- catch a duplicate org-wide assignment (project_id null) alongside a
-- project-scoped one, since Postgres treats every null as distinct in a
-- unique constraint — this expression index (coalescing null to a
-- sentinel) is what table-level PRIMARY KEY/UNIQUE can't express.
create unique index role_assignments_uidx
    on role_assignments (user_id, role_id, coalesce(project_id, '00000000-0000-0000-0000-000000000000'));

create table projects (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    branch_id uuid references branches(id),
    name text not null,
    code text not null,
    location text not null,
    original_title_number text,
    total_size numeric(14, 4) not null,
    area_unit text not null check (area_unit in ('hectares', 'acres', 'square_metres')),
    status text not null default 'planning'
        check (status in ('planning', 'active', 'on_hold', 'sold_out', 'closed')),
    assigned_manager_id uuid references users(id),
    created_at timestamptz not null default now(),
    unique (organization_id, code)
);
alter table role_assignments
    add constraint role_assignments_project_fk foreign key (project_id) references projects(id);

create table project_map_versions (
    id uuid primary key default gen_random_uuid(),
    project_id uuid not null references projects(id),
    version_number int not null,
    status text not null default 'draft'
        check (status in ('draft', 'pending_approval', 'approved', 'published', 'superseded')),
    -- Object storage key/path (Railway volume or S3-compatible bucket —
    -- see docs/10-database-and-security-design.md), not a public URL.
    source_document_path text not null,
    polygons jsonb not null default '{}',
    uploaded_by uuid not null references users(id),
    approved_by uuid references users(id),
    created_at timestamptz not null default now(),
    unique (project_id, version_number)
);

create table plot_status_config (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    status_key text not null,
    label text not null,
    color text not null,
    sort_order int not null default 0,
    unique (organization_id, status_key)
);

create table plots (
    id uuid primary key default gen_random_uuid(),
    project_id uuid not null references projects(id),
    plot_number text not null,
    title_number text,
    size numeric(14, 4) not null,
    asking_price numeric(16, 2) not null,
    minimum_price numeric(16, 2) not null,
    status text not null default 'available',
    map_feature_id text,
    assigned_customer_id uuid,
    created_at timestamptz not null default now(),
    unique (project_id, plot_number)
);
create index plots_project_id_idx on plots(project_id);
create index plots_status_idx on plots(status);

create table customers (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    full_name text not null,
    email text,
    phone text,
    id_number text,
    assigned_agent_id uuid references users(id),
    created_at timestamptz not null default now()
);
alter table plots
    add constraint plots_assigned_customer_fk foreign key (assigned_customer_id) references customers(id);

create table plot_sales (
    id uuid primary key default gen_random_uuid(),
    plot_id uuid not null references plots(id),
    customer_id uuid not null references customers(id),
    organization_id uuid not null references organizations(id),
    agent_id uuid references users(id),
    payment_mode text not null
        check (payment_mode in ('full_cash', 'lipa_pole_pole_interest_free', 'lipa_pole_pole_interest_bearing')),
    agreed_price numeric(16, 2) not null,
    created_at timestamptz not null default now()
);
-- One active sale per plot. Once cancelled/repossessed sales are modelled
-- with a status column, replace this with a partial unique index over
-- active statuses only, so a cancelled sale can be superseded by a new one.
create unique index plot_sales_one_active_per_plot on plot_sales(plot_id);

create table plot_loan_accounts (
    id uuid primary key default gen_random_uuid(),
    account_number text not null unique,
    sale_id uuid not null references plot_sales(id),
    principal numeric(16, 2) not null,
    interest_rate numeric(6, 4),
    deposit_required numeric(16, 2) not null default 0,
    deposit_paid numeric(16, 2) not null default 0,
    instalment_amount numeric(16, 2) not null,
    repayment_frequency_days int not null,
    start_date date not null,
    status text not null default 'draft',
    amount_paid numeric(16, 2) not null default 0,
    outstanding_balance numeric(16, 2) not null default 0,
    days_in_arrears int not null default 0
);

create table repayment_schedule_entries (
    id uuid primary key default gen_random_uuid(),
    loan_account_id uuid not null references plot_loan_accounts(id),
    instalment_number int not null,
    due_date date not null,
    principal_due numeric(16, 2) not null,
    interest_due numeric(16, 2) not null default 0,
    fees_due numeric(16, 2) not null default 0,
    total_due numeric(16, 2) not null,
    amount_paid numeric(16, 2) not null default 0,
    status text not null default 'upcoming',
    unique (loan_account_id, instalment_number)
);

create table payments (
    id uuid primary key default gen_random_uuid(),
    loan_account_id uuid not null references plot_loan_accounts(id),
    amount numeric(16, 2) not null,
    payment_date date not null,
    method text not null,
    external_reference text,
    status text not null default 'captured',
    captured_by uuid not null references users(id),
    verified_by uuid references users(id),
    created_at timestamptz not null default now()
);
create index payments_loan_account_id_idx on payments(loan_account_id);

create table audit_log (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    actor_id uuid references users(id),
    entity_type text not null,
    entity_id uuid not null,
    action text not null,
    before_state jsonb,
    after_state jsonb,
    created_at timestamptz not null default now()
);
create index audit_log_entity_idx on audit_log(entity_type, entity_id);

-- =========================================================================
-- Row-Level Security — defense in depth behind the backend's own
-- authorization checks (see the header note above). Every tenant-scoped
-- table is locked down by default and opened only to rows matching the
-- session-local org id the backend sets per transaction. The backend's
-- system/admin connection pool (migrations, Paystack webhook processing —
-- see database/migrations/0002_billing.sql) uses a Postgres role with
-- BYPASSRLS, which is a role grant, not a policy, and is documented in
-- docs/10-database-and-security-design.md.
-- =========================================================================

alter table organizations enable row level security;
alter table branches enable row level security;
alter table roles enable row level security;
alter table users enable row level security;
alter table role_assignments enable row level security;
alter table projects enable row level security;
alter table project_map_versions enable row level security;
alter table plot_status_config enable row level security;
alter table plots enable row level security;
alter table customers enable row level security;
alter table plot_sales enable row level security;
alter table plot_loan_accounts enable row level security;
alter table repayment_schedule_entries enable row level security;
alter table payments enable row level security;
alter table audit_log enable row level security;

create policy org_isolation_select on organizations for select
    using (id = public.current_org_id());

create policy users_org_select on users for select
    using (organization_id = public.current_org_id());

-- Every other tenant-scoped table follows the same shape: read/write only
-- rows in the caller's organization. Write policies are deliberately
-- read-only (`for select`) at this stage — INSERT/UPDATE/DELETE policies
-- are added per table as each write workflow is built, matched to the
-- backend's actual permission model for that action (e.g. plot price
-- changes going through an approval-aware check) rather than a blanket
-- "same org can write anything" rule.
create policy branches_org_select on branches for select
    using (organization_id = public.current_org_id());
create policy roles_org_select on roles for select
    using (organization_id = public.current_org_id());
create policy role_assignments_org_select on role_assignments for select
    using (user_id in (select id from users where organization_id = public.current_org_id()));
create policy projects_org_select on projects for select
    using (organization_id = public.current_org_id());
create policy project_map_versions_org_select on project_map_versions for select
    using (project_id in (select id from projects where organization_id = public.current_org_id()));
create policy plot_status_config_org_select on plot_status_config for select
    using (organization_id = public.current_org_id());
create policy plots_org_select on plots for select
    using (project_id in (select id from projects where organization_id = public.current_org_id()));
create policy customers_org_select on customers for select
    using (organization_id = public.current_org_id());
create policy plot_sales_org_select on plot_sales for select
    using (organization_id = public.current_org_id());
create policy plot_loan_accounts_org_select on plot_loan_accounts for select
    using (sale_id in (select id from plot_sales where organization_id = public.current_org_id()));
create policy repayment_schedule_org_select on repayment_schedule_entries for select
    using (loan_account_id in (
        select pla.id from plot_loan_accounts pla
        join plot_sales ps on ps.id = pla.sale_id
        where ps.organization_id = public.current_org_id()
    ));
create policy payments_org_select on payments for select
    using (loan_account_id in (
        select pla.id from plot_loan_accounts pla
        join plot_sales ps on ps.id = pla.sale_id
        where ps.organization_id = public.current_org_id()
    ));
create policy audit_log_org_select on audit_log for select
    using (organization_id = public.current_org_id());
