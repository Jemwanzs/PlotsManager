//! (label, hex color) for the two status enums the UI renders as colour-
//! coded badges. Both `frontend` (for `api::mock`) and `backend` (for
//! real API responses) need the *same* mapping — see the module docs on
//! `api_types` for why that means it lives here, not in either crate.

use crate::{ApprovalStatus, LeadStage, LoanAccountStatus, PlotStatus, QuotationStatus};

/// Matches the suggested defaults in docs/05-project-and-plot-management.md.
/// Organisations can reconfigure both per docs/05 once `plot_status_config`
/// is exposed through the real API — these are the shipped defaults, not
/// a hardcoded final answer.
pub fn plot_status_meta(status: PlotStatus) -> (&'static str, &'static str) {
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

pub fn loan_status_meta(status: LoanAccountStatus) -> (&'static str, &'static str) {
    match status {
        LoanAccountStatus::Draft => ("Draft", "#6b7280"),
        LoanAccountStatus::PendingApproval => ("Pending Approval", "#d97706"),
        LoanAccountStatus::ApprovedAwaitingDeposit => ("Awaiting Deposit", "#eab308"),
        LoanAccountStatus::ActiveCurrent => ("Active", "#16a34a"),
        LoanAccountStatus::ActivePartiallyPaid => ("Active (Partially Paid)", "#38bdf8"),
        LoanAccountStatus::InGracePeriod => ("In Grace Period", "#f97316"),
        LoanAccountStatus::InArrears => ("In Arrears", "#dc2626"),
        LoanAccountStatus::Restructured => ("Restructured", "#9333ea"),
        LoanAccountStatus::SettlementPendingVerification => ("Settlement Pending", "#1d4ed8"),
        LoanAccountStatus::FullyPaid => ("Fully Paid", "#15734f"),
        LoanAccountStatus::Cancelled => ("Cancelled", "#78350f"),
        LoanAccountStatus::Defaulted => ("Defaulted", "#7f1d1d"),
        LoanAccountStatus::RepossessedOrReallocated => ("Repossessed", "#111827"),
        LoanAccountStatus::Closed => ("Closed", "#6b7280"),
    }
}

pub fn lead_stage_meta(stage: LeadStage) -> (&'static str, &'static str) {
    match stage {
        LeadStage::New => ("New", "#38bdf8"),
        LeadStage::Contacted => ("Contacted", "#eab308"),
        LeadStage::SiteVisit => ("Site Visit", "#f97316"),
        LeadStage::Negotiating => ("Negotiating", "#9333ea"),
        LeadStage::Lost => ("Lost", "#6b7280"),
    }
}

/// `is_expired` is computed by the caller (a `Sent` quotation past its
/// `valid_until` — see `Quotation`'s module docs) since this crate has
/// no clock/IO of its own to derive it from.
pub fn quotation_status_meta(status: QuotationStatus, is_expired: bool) -> (&'static str, &'static str) {
    if is_expired && status == QuotationStatus::Sent {
        return ("Expired", "#6b7280");
    }
    match status {
        QuotationStatus::Draft => ("Draft", "#6b7280"),
        QuotationStatus::Sent => ("Sent", "#38bdf8"),
        QuotationStatus::Accepted => ("Accepted", "#16a34a"),
        QuotationStatus::Rejected => ("Rejected", "#dc2626"),
    }
}

pub fn approval_status_meta(status: ApprovalStatus) -> (&'static str, &'static str) {
    match status {
        ApprovalStatus::Pending => ("Pending", "#eab308"),
        ApprovalStatus::Approved => ("Approved", "#16a34a"),
        ApprovalStatus::Rejected => ("Rejected", "#dc2626"),
    }
}
