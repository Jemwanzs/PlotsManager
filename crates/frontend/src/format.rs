use domain::{LedgerEntryType, PaymentMode, PaymentStatus};
use rust_decimal::Decimal;

pub fn format_payment_mode(mode: PaymentMode) -> &'static str {
    match mode {
        PaymentMode::FullCash => "Full cash",
        PaymentMode::LipaPolePoleInterestFree => "Lipa Pole Pole (interest-free)",
        PaymentMode::LipaPolePoleInterestBearing => "Lipa Pole Pole (interest-bearing)",
    }
}

pub fn format_payment_status(status: PaymentStatus) -> &'static str {
    match status {
        PaymentStatus::Captured => "Captured",
        PaymentStatus::Verified => "Verified",
        PaymentStatus::Posted => "Posted",
        PaymentStatus::Rejected => "Rejected",
        PaymentStatus::Reversed => "Reversed",
    }
}

pub fn format_ledger_entry_type(entry_type: LedgerEntryType) -> &'static str {
    match entry_type {
        LedgerEntryType::Payment => "Payment",
        LedgerEntryType::ChargeInterest => "Interest Charge",
        LedgerEntryType::ChargePenalty => "Penalty Charge",
        LedgerEntryType::WaiverInterest => "Interest Waiver",
        LedgerEntryType::WaiverPenalty => "Penalty Waiver",
        LedgerEntryType::Reversal => "Reversal",
        LedgerEntryType::Adjustment => "Adjustment",
    }
}

/// "KES 1,234,500" — `Decimal`'s own `Display` has no thousands
/// separator, and every money value in this app needs one. The space
/// is a non-breaking one: a narrow stat tile will otherwise wrap
/// right after the currency code, stranding it alone on its own line
/// above the number. `currency` is the organization's configured
/// currency (`use_currency()`, populated from `GET /api/v1/settings`
/// after login) — never hardcoded, so a non-KES tenant sees their own
/// currency everywhere this is called.
pub fn format_money(amount: Decimal, currency: &str) -> String {
    let sign = if amount.is_sign_negative() { "-" } else { "" };
    format!("{sign}{currency}\u{a0}{}", format_amount(amount.abs()))
}

/// The bare number, no currency code — for places that show the
/// currency once nearby instead of repeating it on every figure (the
/// dashboard's stat cards; see `pages/dashboard.rs`).
pub fn format_amount(amount: Decimal) -> String {
    let sign = if amount.is_sign_negative() { "-" } else { "" };
    let magnitude = amount.abs().round();
    format!("{sign}{}", group_thousands(&magnitude.to_string()))
}

fn group_thousands(digits: &str) -> String {
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in bytes.iter().enumerate() {
        if i != 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*ch as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_thousands_separators() {
        assert_eq!(format_money(Decimal::from(1234500), "KES"), "KES\u{a0}1,234,500");
        assert_eq!(format_money(Decimal::from(500), "KES"), "KES\u{a0}500");
        assert_eq!(format_money(Decimal::from(-42000), "KES"), "-KES\u{a0}42,000");
        assert_eq!(format_money(Decimal::from(0), "KES"), "KES\u{a0}0");
        assert_eq!(format_money(Decimal::from(1234500), "USD"), "USD\u{a0}1,234,500");
    }

    #[test]
    fn formats_bare_amounts() {
        assert_eq!(format_amount(Decimal::from(1234500)), "1,234,500");
        assert_eq!(format_amount(Decimal::from(-42000)), "-42,000");
    }
}
