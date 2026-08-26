-- Phase 2 foundation schema: organisations, users/roles, projects, plots,
-- customers. Every tenant-scoped table carries organization_id directly (or
-- transitively via project_id/plot_id) so row-level isolation can be
-- enforced at the query layer and, later, via Postgres RLS policies.

create extension if not exists "pgcrypto";

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

create table users (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    branch_id uuid references branches(id),
    full_name text not null,
    email text not null,
    password_hash text not null,
    is_active boolean not null default true,
    created_at timestamptz not null default now(),
    unique (organization_id, email)
);

create table role_assignments (
    user_id uuid not null references users(id),
    role_id uuid not null references roles(id),
    project_id uuid,
    branch_id uuid references branches(id),
    primary key (user_id, role_id, coalesce(project_id, '00000000-0000-0000-0000-000000000000'))
);

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
    source_document_url text not null,
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
