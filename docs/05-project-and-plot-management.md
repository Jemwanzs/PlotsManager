# 05 — Project and Plot Management

## Organisation configuration

Each organisation configures: name/code, branches/offices, regions/project
locations, currency and taxation rules, project numbering, plot numbering,
pricing rules, booking/reservation rules, commission rules, payment plans,
approval workflows, roles/access rights, notification templates, document
templates, security/audit settings.

## Land project

Fields: name, unique project code, location, GPS coordinates/boundary,
original land title or parcel number, total size, unit (hectares / acres /
square metres), purchase/ownership details, surveyor/legal information,
status, plot count, roads/utilities/public/reserved areas, project plan or
survey document, pricing/payment plans, supporting documents, assigned
project manager, assigned sales team. Modelled in `domain::Project`
([crates/domain/src/project.rs](../crates/domain/src/project.rs)).

## Individual plots

Every plot is a distinct inventory item: internal plot ID (system-generated,
immutable — assigned even before a subdivision title exists), project plot
number, title deed/parcel number where available, tenant-issued unique
code, size, asking price, minimum acceptable price, status, map
coordinates/polygon, road frontage/access, amenities, assigned customer,
booking/sales history, payment status, transfer status, documents and
approvals. Modelled in `domain::Plot`
([crates/domain/src/plot.rs](../crates/domain/src/plot.rs)).

A title deed number is unique *when one exists*, but the plot's internal ID
is the durable identity the rest of the system references.

## Plot statuses and colour coding

Configurable per organisation (`plot_status_config` table — label, colour,
sort order per status key), but the underlying state machine is fixed so
workflow/approval logic has something stable to hang off:

| Status | Suggested colour | Meaning |
|---|---|---|
| Available | Green | Open for sale |
| Selected | Light blue | Customer has expressed interest |
| Temporarily Held | Yellow | Short internal hold |
| Reserved | Orange | Booking initiated |
| Booked | Purple | Booking requirements completed |
| Under Approval | Amber | Transaction awaiting approval |
| Sold | Red | Sale completed |
| Transfer in Progress | Dark blue | Title transfer underway |
| Transferred | Grey | Ownership transferred |
| Blocked | Black | Not available for sale |
| Disputed | Maroon | Legal or ownership issue |
| Cancelled / Repossessed | Brown | Previous allocation reversed |

Administrators configure labels, colours, allowed transitions, and who may
perform each transition (ties into [09](09-approval-workflows.md)). See
`domain::PlotStatus` for the enum.

## Key invariants

- A plot cannot be assigned to two active customers simultaneously (unless
  joint ownership is explicitly configured).
- Plot numbers, title numbers, and internal IDs are checked for duplicates
  at creation time.
- Price and status changes must update the plot register and the published
  map consistently — never one without the other.

## Legacy reality (see [02](02-existing-vba-system-analysis.md))

The legacy workbook has **no plot number field at all** — a free-text
"Plot Description" is the plot's only identity, and it must be unique
*across the entire workbook*, not per project. There is no minimum price
field, no polygon/coordinate data, and a plot's "status" is implicit
(present in the loan register = sold; a manual "resale" override bypasses
the duplicate check to re-sell a repossessed plot rather than transitioning
it through a real status). The system-generated UUID + configurable
per-project human code design above is a deliberate fix for the specific
failure mode this caused (plot codes colliding or being reused informally
across projects).
