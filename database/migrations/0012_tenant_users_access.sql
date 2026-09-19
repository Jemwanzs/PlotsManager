-- Settings -> Users & Access (Tenant Admin user management). Adds what
-- the user list/detail needs that `users` didn't carry yet — everything
-- else (role assignment, branch assignment) already had a column
-- (`role_assignments`, `users.branch_id`) from 0001_init.sql.

alter table users
    add column mobile text,
    add column last_login_at timestamptz;
