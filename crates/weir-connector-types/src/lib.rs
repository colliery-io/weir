//! # weir-connector-types
//!
//! The connector contract's boundary types, error taxonomy, and raw-envelope
//! codec — with **no `fidius` dependency**, so this crate builds for
//! `wasm32-wasip2` and is shared by the host/dylib interface ([`weir-connector`])
//! and WASM-guest connectors alike.
//!
//! The `wit` feature enables `#[derive(WitType)]` on the typed-method types so a
//! WASM guest can project them to WIT (WEIR-T-0002/0003).

pub mod error;
pub mod mssql;
pub mod snowflake;
pub mod types;

pub use error::{ConnectorError, ErrorKind};
pub use types::*;

use serde::{Deserialize, Serialize};

/// Outcome of `Connector::discover`. An explicit enum (rather than
/// `Result<Catalog, ConnectorError>`) keeps the typed return independent of any
/// host-side `Result` special-casing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum DiscoverOutcome {
    Catalog(Catalog),
    Error(ConnectorError),
}
