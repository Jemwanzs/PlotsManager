//! Shared domain model for Real Estate Manager.
//!
//! This crate has no I/O — it is pure data and enums shared between the
//! `backend` (Axum API) and `frontend` (Leptos/WASM) crates, so both
//! sides agree on shapes without duplicating type definitions.
//! `api_types` specifically is the request/response wire contract
//! between them (see its module docs); the rest mirror the database
//! schema (`database/migrations/`).

pub mod api_types;
pub mod billing;
pub mod customer;
pub mod organization;
pub mod plot;
pub mod project;
pub mod sales;
pub mod status_meta;
pub mod user;

pub use api_types::*;
pub use billing::*;
pub use customer::*;
pub use organization::*;
pub use plot::*;
pub use project::*;
pub use sales::*;
pub use status_meta::*;
pub use user::*;
