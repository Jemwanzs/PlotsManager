# 06 — Interactive Map Engine

## Approach: AI-assisted, not automatic

Converting a scanned/PDF project plan into an interactive map is an
**AI-assisted workflow with mandatory human verification**, not
uncontrolled automatic conversion. Scanned Kenyan land plans vary too much
in quality (faint boundaries, handwritten plot numbers, overlapping
labels, irregular plots, rotated/damaged scans) to trust unreviewed.

Build order deliberately puts manual mapping first (Phase 3) and
AI-assisted conversion later (Phase 4) — see
[14](14-development-roadmap.md) — so there's a reliable operational system
immediately while the conversion model is trained/tested against real plan
formats.

## Conversion pipeline (Phase 4)

1. Accept the original PDF/scan/high-res image.
2. Enhance clarity, deskew, denoise.
3. Detect plot boundaries, roads, labels, dimensions, plot numbers.
4. OCR the visible plot numbers/text.
5. Convert each detected plot into a polygon, overlaid on the original.
6. **Review screen** — humans correct boundaries and labels.
7. Match each polygon to its plot record.
8. Submit for approval.
9. Publish as the interactive project map.

## Manual verification / editing (Phase 3 — build first)

Users must be able to: draw or redraw polygon boundaries, move polygon
points, split or merge detected plots, correct plot numbers, mark
roads/public spaces, link polygons to existing plot records, flag
missing/duplicated plots, compare the overlay against the original
document side by side, and approve the final map before publication.

## Interactive experience (once published)

- Every plot is clickable/tappable; hover/tap shows number, size, price,
  status.
- Colours update automatically as plot status changes.
- Filter by size, price, section, location.
- A controlled public version can be shown to customers.
- Selecting a plot can initiate a lead, hold, reservation, or booking.
- Managers can open the full customer/payment/approval/document history
  from a plot.
- Sold plots can be hidden from marketers while staying visible to
  authorised managers (ties into [04](04-user-roles-and-permissions.md)).
- The original uploaded document stays available for comparison.

## v1 mapping technology

Start simple, not with full GIS:

- The uploaded plan as a background image layer.
- SVG or GeoJSON polygons overlaid on it (`project_map_versions.polygons`
  is a GeoJSON `FeatureCollection` — see
  [10](10-database-and-security-design.md#schema)).
- Unique plot IDs (`plots.map_feature_id`) connecting polygons to plot
  records.
- Pan, zoom, search, filter, colour-coded statuses.
- A polygon editor for human verification.

Later phases can add GPS coordinates, satellite maps, georeferencing, GIS
layers, survey coordinates, drone imagery, and Google Maps/Mapbox/OSM
integration.

## Source-of-truth controls

- Every published map has a version number (`project_map_versions.version_number`).
- The original uploaded plan is never overwritten.
- Every edit creates a new **draft** version; the currently approved
  version stays locked and in force until a new one is approved.
- Every plot-boundary change is audited (who uploaded, reviewed, approved,
  published).
- Sales activity uses only the currently **published** map version.
- Previous versions remain accessible to authorised users.

**Important**: the approved digital map is the platform's *operational*
source of truth. It is never the *legal* source of truth — registered
survey documents, title records, and the land registry remain
authoritative.
