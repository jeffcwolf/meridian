//! Server-side reads from the SQLite cache produced by the Python pipeline.
//! Compiled only in the `ssr` build (rusqlite is not available in wasm).
//!
//! Each submodule is one cohesive read surface:
//!   - `reads`      — company summaries, detail facts, and export
//!   - `compare`    — the cross-country comparator and FX conversion
//!   - `dashboards` — coverage, data-quality and extension aggregates
//!   - `format`     — amount parsing and display formatting

mod compare;
mod dashboards;
mod format;
mod reads;

#[cfg(test)]
mod tests;

use rusqlite::Connection;

pub(crate) use compare::compare;
pub(crate) use dashboards::{coverage, extension_summary, quality_summary};
pub(crate) use reads::{company_export, list_companies, load_company};

fn db_path() -> String {
    std::env::var("MERIDIAN_DB").unwrap_or_else(|_| "data/meridian.db".to_string())
}

/// Open the SQLite cache. The path is `data/meridian.db`, overridable via the
/// `MERIDIAN_DB` environment variable.
pub(crate) fn open_db() -> rusqlite::Result<Connection> {
    Connection::open(db_path())
}
