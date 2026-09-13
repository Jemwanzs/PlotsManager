use domain::LoanAccountStatus;

/// (label, hex color) for each Plot Loan Account status — same pattern as
/// `plot_status::status_meta`, kept separate since the two enums are
/// unrelated and shouldn't be conflated just because both render badges.
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
