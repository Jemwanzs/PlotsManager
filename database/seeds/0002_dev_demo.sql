-- Local/dev-only demo data — mirrors the frontend's mock dataset
-- (crates/frontend/src/api/mock.rs) so the real backend has something to
-- show and the login screen's documented demo credentials work against
-- either. Never run this against a production database.
--
-- Login: admin@acaciagrove.example / password123
-- (hash regenerated via `cargo test -p backend print_dev_seed_hash --
-- --nocapture --ignored` in crates/backend/src/auth.rs if the password
-- above ever changes)

with org as (
    insert into organizations (name, code, currency)
    values ('Acacia Grove Properties', 'ACACIA', 'KES')
    returning id
),
admin_role as (
    insert into roles (organization_id, name, permissions)
    select id, 'Admin', '["*"]'::jsonb from org
    returning id, organization_id
),
demo_user as (
    insert into users (organization_id, full_name, email, password_hash)
    select org.id, 'Amina Wanjiru', 'admin@acaciagrove.example',
        '$argon2id$v=19$m=19456,t=2,p=1$8yD50bYj/PH6QCGqGKvZzw$nAz/tXA3adrxEPTubC3sT8puh4v0r1A8mRbMbtSXSbs'
    from org
    returning id, organization_id
)
insert into role_assignments (user_id, role_id)
select demo_user.id, admin_role.id from demo_user, admin_role;

with org as (select id from organizations where code = 'ACACIA'),
manager as (select id from users where email = 'admin@acaciagrove.example'),
proj1 as (
    insert into projects (organization_id, name, code, location, total_size, area_unit, status, assigned_manager_id)
    select org.id, 'Acacia Grove — Phase I', 'AG-P1', 'Kitengela, Kajiado', 20, 'acres', 'active', manager.id
    from org, manager
    returning id
),
proj2 as (
    insert into projects (organization_id, name, code, location, total_size, area_unit, status, assigned_manager_id)
    select org.id, 'Riverside Meadows', 'RM', 'Malaa, Machakos', 15, 'acres', 'active', manager.id
    from org, manager
    returning id
)
insert into plots (project_id, plot_number, size, asking_price, minimum_price, status)
select proj1.id, 'AG-P1-' || lpad(n::text, 3, '0'), 1.25,
    650000 + (n % 5) * 35000, 600000 + (n % 5) * 35000,
    (array['available','available','available','selected','temporarily_held',
           'reserved','booked','under_approval','sold','transfer_in_progress',
           'transferred','blocked','disputed','cancelled'])[((n - 1) % 14) + 1]
from proj1, generate_series(1, 16) as n
union all
select proj2.id, 'RM-' || lpad(n::text, 3, '0'), 1.0,
    720000 + (n % 5) * 40000, 670000 + (n % 5) * 40000,
    (array['available','available','available','selected','temporarily_held',
           'reserved','booked','under_approval','sold','transfer_in_progress',
           'transferred','blocked','disputed','cancelled'])[((n - 1) % 14) + 1]
from proj2, generate_series(1, 12) as n;

insert into customers (organization_id, full_name, email, phone, id_number, assigned_agent_id)
select org.id, c.full_name, c.email, c.phone, c.id_number, manager.id
from (select id from organizations where code = 'ACACIA') org,
     (select id from users where email = 'admin@acaciagrove.example') manager,
     (values
        ('James Otieno', 'j.otieno@example.com', '0722 000 111', '29889001'),
        ('Grace Mumbi', 'grace.mumbi@example.com', '0733 222 444', '30112233')
     ) as c(full_name, email, phone, id_number);
