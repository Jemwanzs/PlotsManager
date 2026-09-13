use domain::{PaymentMode, PaymentStatus};
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

/// "KES 1,234,500" — `Decimal`'s own `Display` has no thousands
/// separator, and every money value in this app needs one.
pub fn format_kes(amount: Decimal) -> String {
    let sign = if amount.is_sign_negative() { "-" } else { "" };
    let magnitude = amount.abs().round();
    format!("{sign}KES {}", group_thousands(&magnitude.to_string()))
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
        assert_eq!(format_kes(Decimal::from(1234500)), "KES 1,234,500");
        assert_eq!(format_kes(Decimal::from(500)), "KES 500");
        assert_eq!(format_kes(Decimal::from(-42000)), "-KES 42,000");
        assert_eq!(format_kes(Decimal::from(0)), "KES 0");
    }
}
