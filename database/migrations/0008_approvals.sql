-- A deliberately small slice of docs/09's full N-level approval-workflow
-- engine (org-configurable approver chains, sequential/parallel steps,
-- delegation/escalation — none of that exists in schema anywhere yet).
-- This is one trigger (agreed/quoted price below the plot's
-- minimum_price — docs/07: "selling below the configured minimum...
-- must trigger the appropriate approval chain, rather than being
-- silently allowed"), one step, no scoped approver assignment: any
-- authenticated user other than the requester may decide it, since
-- every user today is an unrestricted 'Admin' (see
-- crates/backend/src/routes/auth.rs's signup handler) — real
-- approver-role scoping via the already-provisioned but currently-
-- unused `roles`/`role_assignments` tables is future work, not
-- inferable from any spec or legacy precedent (docs/09's own module
-- notes flag separation-of-duties as "a product decision, not
-- something inferable from history").
--
-- Storing the would-be sale's parameters (not just a subject_id
-- pointing at an already-created row) is what lets this gate BOTH
-- a direct reservation (`routes/sales.rs::create_sale`, where no
-- `plot_sales` row exists yet at request time) and a quotation
-- acceptance (`routes/quotations.rs::accept_quotation`, where
-- `quotation_id` is set for traceability) with one table — see
-- `crates/backend/src/routes/approvals.rs`'s `gate_price`.
create table approval_requests (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    plot_id uuid not null references plots(id),
    customer_id uuid not null references customers(id),
    agent_id uuid references users(id),
    payment_mode text not null
        check (payment_mode in ('full_cash', 'lipa_pole_pole_interest_free', 'lipa_pole_pole_interest_bearing')),
    agreed_price numeric(16, 2) not null,
    minimum_price numeric(16, 2) not null,
    quotation_id uuid references quotations(id),
    requested_by uuid not null references users(id),
    reason text not null,
    status text not null default 'pending' check (status in ('pending', 'approved', 'rejected')),
    decided_by uuid references users(id),
    decided_at timestamptz,
    decision_notes text,
    -- Set once an approved request's gated sale actually goes through
    -- (a request can be approved but never acted on, e.g. the agent
    -- never retries) — lets `gate_price` tell "approved, ready to
    -- consume" apart from "approved and already consumed".
    resulting_sale_id uuid references plot_sales(id),
    created_at timestamptz not null default now()
);
create index approval_requests_organization_id_idx on approval_requests(organization_id);
create index approval_requests_status_idx on approval_requests(organization_id, status);

alter table approval_requests enable row level security;
create policy approval_requests_org_select on approval_requests for select
    using (organization_id = public.current_org_id());
