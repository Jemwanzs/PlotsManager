//! Shared domain model for Real Estate Manager.
//!
//! This crate has no I/O — it is pure data and enums shared between the
//! `api` (Axum backend) and `frontend` (Leptos/WASM) crates, so both sides
//! agree on shapes without duplicating type definitions.

pub mod billing;
pub mod customer;
pub mod organization;
pub mod plot;
pub mod project;
pub mod sales;
pub mod user;

pub use billing::*;
pub use customer::*;
pub use organization::*;
pub use plot::*;
pub use project::*;
pub use sales::*;
pub use user::*;
