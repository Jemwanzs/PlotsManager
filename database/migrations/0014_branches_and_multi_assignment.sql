-- Settings -> Branches (full CRUD: the table has existed since day one
-- but only ever had 3 bare columns and zero rows) and multi-branch user
-- assignment (a user belongs to one tenant but any number of branches
-- within it — 0001_init.sql's `users.branch_id` was always a single
-- FK, which can't express that).

alter table branches
    add column location text,
    add column contact_name text,
    add column contact_phone text,
    add column manager_id uuid references users(id),
    add column is_active boolean not null default true;

-- `users.branch_id` stays as-is (nothing that already reads it needs to
-- change) and now means "primary branch" — kept in sync with this
-- table's one `is_primary = true` row per user (crates/backend/src/
-- routes/users.rs), rather than being replaced by it, so a user
-- created before this migration with a single branch_id keeps working
-- unchanged until someone edits their branch assignment.
create table user_branches (
    user_id uuid not null references users(id),
    branch_id uuid not null references branches(id),
    is_primary boolean not null default false,
    primary key (user_id, branch_id)
);
create unique index user_branches_one_primary_idx
    on user_branches (user_id) where is_primary;

-- Backfill: every existing user with a single branch_id gets that one
-- row, marked primary, so nobody's assignment silently disappears the
-- moment this migration runs.
insert into user_branches (user_id, branch_id, is_primary)
select id, branch_id, true from users where branch_id is not null;
