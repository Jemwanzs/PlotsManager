//! Client-side CSV parsing for bulk imports (tenant onboarding) — see
//! `pages/project_detail.rs`'s `BulkPlotImport` and
//! `pages/customers_list.rs`'s `BulkCustomerImport`. Parsing happens
//! here, in the browser, so a user sees per-row problems (a blank
//! required field, text where a price belongs) before anything is
//! sent to the server — the server still re-validates everything
//! itself (`crates/backend/src/routes/projects.rs::insert_plot`,
//! `routes/customers.rs::insert_customer`), since a parsed row can
//! still fail there (a duplicate plot number, an ID already on file).
//!
//! Columns are read positionally, not by header name — the header row
//! is just skipped (via `csv`'s default `has_headers`), so what
//! matters is column *order* matching the downloadable template, not
//! its exact text.

use std::str::FromStr;

use chrono::NaiveDate;
use domain::{BulkSaleRow, CreateCustomerInput, CreatePlotInput, PaymentMode};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Clone)]
pub struct ParsedRow<T> {
    pub row: u32,
    pub result: Result<T, String>,
}

pub const PLOTS_TEMPLATE: &str = "plot_number,size,side_1,side_2,asking_price,minimum_price\nAG-P2-001,1.25,80,100,750000,700000\n";

pub const CUSTOMERS_TEMPLATE: &str =
    "full_name,email,phone,id_number,source\nJane Wanjiku,jane@example.com,0722000000,12345678,Referral\n";

pub const SALES_TEMPLATE: &str = "project_code,plot_number,customer_lookup,payment_mode,agreed_price,sale_date,amount_paid\n\
AG-P1,AG-P1-003,0722000000,lipa_pole_pole_interest_free,755000,2023-06-01,300000\n\
AG-P1,AG-P1-004,12345678,full_cash,720000,2024-02-14,720000\n";

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Plot side lengths are optional in a bulk import (an onboarding sheet
/// often has acreage but not side measurements) — a blank cell means "not
/// recorded", same as skipping the field in the "Add a plot" form; only
/// a non-blank, non-numeric cell is a row error.
fn parse_optional_dimension(cell: Option<&str>, field: &str) -> Result<Option<Decimal>, String> {
    match cell.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => Decimal::from_str(s)
            .map(Some)
            .map_err(|_| format!("{field} must be a number")),
    }
}

pub fn parse_plots_csv(text: &str, project_id: Uuid) -> Vec<ParsedRow<CreatePlotInput>> {
    let mut reader = csv::ReaderBuilder::new().trim(csv::Trim::All).from_reader(text.as_bytes());
    reader
        .records()
        .enumerate()
        .map(|(idx, record)| {
            let row = idx as u32 + 1;
            let result = (|| -> Result<CreatePlotInput, String> {
                let record = record.map_err(|e| format!("couldn't read this row: {e}"))?;
                let plot_number = record.get(0).unwrap_or("").trim();
                if plot_number.is_empty() {
                    return Err("plot_number is required".to_string());
                }
                let size = Decimal::from_str(record.get(1).unwrap_or("").trim())
                    .map_err(|_| "size must be a number".to_string())?;
                let side_1 = parse_optional_dimension(record.get(2), "side_1")?;
                let side_2 = parse_optional_dimension(record.get(3), "side_2")?;
                let asking_price = Decimal::from_str(record.get(4).unwrap_or("").trim())
                    .map_err(|_| "asking_price must be a number".to_string())?;
                let minimum_price = Decimal::from_str(record.get(5).unwrap_or("").trim())
                    .map_err(|_| "minimum_price must be a number".to_string())?;
                Ok(CreatePlotInput {
                    project_id,
                    plot_number: plot_number.to_string(),
                    size,
                    side_1,
                    side_2,
                    asking_price,
                    minimum_price,
                })
            })();
            ParsedRow { row, result }
        })
        .collect()
}

pub fn parse_customers_csv(text: &str) -> Vec<ParsedRow<CreateCustomerInput>> {
    let mut reader = csv::ReaderBuilder::new().trim(csv::Trim::All).from_reader(text.as_bytes());
    reader
        .records()
        .enumerate()
        .map(|(idx, record)| {
            let row = idx as u32 + 1;
            let result = (|| -> Result<CreateCustomerInput, String> {
                let record = record.map_err(|e| format!("couldn't read this row: {e}"))?;
                let full_name = record.get(0).unwrap_or("").trim();
                if full_name.is_empty() {
                    return Err("full_name is required".to_string());
                }
                Ok(CreateCustomerInput {
                    full_name: full_name.to_string(),
                    email: record.get(1).and_then(non_empty),
                    phone: record.get(2).and_then(non_empty),
                    id_number: record.get(3).and_then(non_empty),
                    source: record.get(4).and_then(non_empty),
                })
            })();
            ParsedRow { row, result }
        })
        .collect()
}

fn parse_payment_mode(s: &str) -> Result<PaymentMode, String> {
    match s.trim() {
        "full_cash" => Ok(PaymentMode::FullCash),
        "lipa_pole_pole_interest_free" => Ok(PaymentMode::LipaPolePoleInterestFree),
        "lipa_pole_pole_interest_bearing" => Ok(PaymentMode::LipaPolePoleInterestBearing),
        other => Err(format!(
            "payment_mode \"{other}\" isn't recognised — use full_cash, \
             lipa_pole_pole_interest_free, or lipa_pole_pole_interest_bearing"
        )),
    }
}

/// A row of *historical* sales — see `domain::BulkSaleRow`'s module
/// docs for why `amount_paid` carries the actual balance already
/// repaid instead of assuming a fresh sale starting at zero.
pub fn parse_sales_csv(text: &str) -> Vec<ParsedRow<BulkSaleRow>> {
    let mut reader = csv::ReaderBuilder::new().trim(csv::Trim::All).from_reader(text.as_bytes());
    reader
        .records()
        .enumerate()
        .map(|(idx, record)| {
            let row = idx as u32 + 1;
            let result = (|| -> Result<BulkSaleRow, String> {
                let record = record.map_err(|e| format!("couldn't read this row: {e}"))?;
                let project_code = record.get(0).unwrap_or("").trim();
                if project_code.is_empty() {
                    return Err("project_code is required".to_string());
                }
                let plot_number = record.get(1).unwrap_or("").trim();
                if plot_number.is_empty() {
                    return Err("plot_number is required".to_string());
                }
                let customer_lookup = record.get(2).unwrap_or("").trim();
                if customer_lookup.is_empty() {
                    return Err(
                        "customer_lookup is required (an existing customer's ID number, phone, or email)"
                            .to_string(),
                    );
                }
                let payment_mode = parse_payment_mode(record.get(3).unwrap_or(""))?;
                let agreed_price = Decimal::from_str(record.get(4).unwrap_or("").trim())
                    .map_err(|_| "agreed_price must be a number".to_string())?;
                let sale_date = NaiveDate::parse_from_str(record.get(5).unwrap_or("").trim(), "%Y-%m-%d")
                    .map_err(|_| "sale_date must be YYYY-MM-DD".to_string())?;
                let amount_paid_field = record.get(6).unwrap_or("").trim();
                let amount_paid = if amount_paid_field.is_empty() {
                    Decimal::ZERO
                } else {
                    Decimal::from_str(amount_paid_field)
                        .map_err(|_| "amount_paid must be a number".to_string())?
                };
                Ok(BulkSaleRow {
                    project_code: project_code.to_string(),
                    plot_number: plot_number.to_string(),
                    customer_lookup: customer_lookup.to_string(),
                    payment_mode,
                    agreed_price,
                    sale_date,
                    amount_paid,
                })
            })();
            ParsedRow { row, result }
        })
        .collect()
}
