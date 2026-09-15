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

use domain::{CreateCustomerInput, CreatePlotInput};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Clone)]
pub struct ParsedRow<T> {
    pub row: u32,
    pub result: Result<T, String>,
}

pub const PLOTS_TEMPLATE: &str = "plot_number,size,asking_price,minimum_price\nAG-P2-001,1.25,750000,700000\n";

pub const CUSTOMERS_TEMPLATE: &str =
    "full_name,email,phone,id_number,source\nJane Wanjiku,jane@example.com,0722000000,12345678,Referral\n";

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
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
                let asking_price = Decimal::from_str(record.get(2).unwrap_or("").trim())
                    .map_err(|_| "asking_price must be a number".to_string())?;
                let minimum_price = Decimal::from_str(record.get(3).unwrap_or("").trim())
                    .map_err(|_| "minimum_price must be a number".to_string())?;
                Ok(CreatePlotInput {
                    project_id,
                    plot_number: plot_number.to_string(),
                    size,
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
