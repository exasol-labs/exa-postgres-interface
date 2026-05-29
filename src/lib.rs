//! Library entry point for `exa-postgres-interface`.
//!
//! The crate ships primarily as a binary, but exposing its modules through
//! `lib.rs` lets integration tests under `tests/` reuse the same types as the
//! binary (notably `AppConfig`, `ExasolSession`, and the pgwire handler /
//! factory) without resorting to `#[path]` includes per test file.
//!
//! Keep this file declaration-only: the actual implementation lives in the
//! individual modules so the binary at `src/main.rs` and the auxiliary
//! binary at `src/bin/exasol_exec.rs` can pick up the same code paths.

pub mod bootstrap;
pub mod config;
pub mod exasol;
pub mod metadata;
pub mod pg_server;
pub mod policy;
pub mod translator;
