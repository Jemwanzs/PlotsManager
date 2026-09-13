use domain::PlotStatus;

/// (label, hex color) for each plot status, matching the suggested
/// defaults in docs/05-project-and-plot-management.md. Organisations can
/// reconfigure both per docs/05 once `plot_status_config` is exposed
/// through the real API — these are the shipped defaults, not a
/// hardcoded final answer.
pub fn status_meta(status: PlotStatus) -> (&'static str, &'static str) {
    match status {
        PlotStatus::Available => ("Available", "#16a34a"),
        PlotStatus::Selected => ("Selected", "#38bdf8"),
        PlotStatus::TemporarilyHeld => ("Temporarily Held", "#eab308"),
        PlotStatus::Reserved => ("Reserved", "#f97316"),
        PlotStatus::Booked => ("Booked", "#9333ea"),
        PlotStatus::UnderApproval => ("Under Approval", "#d97706"),
        PlotStatus::Sold => ("Sold", "#dc2626"),
        PlotStatus::TransferInProgress => ("Transfer in Progress", "#1d4ed8"),
        PlotStatus::Transferred => ("Transferred", "#6b7280"),
        PlotStatus::Blocked => ("Blocked", "#111827"),
        PlotStatus::Disputed => ("Disputed", "#7f1d1d"),
        PlotStatus::Cancelled => ("Cancelled", "#78350f"),
    }
}
