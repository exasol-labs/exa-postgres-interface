# Verification Report: Add Read-Only PostgreSQL Interface

Status update as of 2026-04-27: this report records the original Rust gateway
implementation pass. Later commits expanded the Exasol-side metadata layer,
added full documented PostgreSQL 18 catalog/information-schema relation and
column surface, added DbVisualizer/DBeaver metadata rewrites in
`PG_DEMO.PG_SQL_PREPROCESSOR`, added richer sample data, and updated deployment
docs around systemd operation.

## Summary

Reimplemented the prototype as a Rust binary using the `pgwire` crate instead
of the direct Python wire-protocol server. The active implementation now
provides:

* PostgreSQL startup with cleartext password authentication.
* One direct Exasol WebSocket session per PostgreSQL client connection.
* Exasol TLS support with strict validation, SHA-256 certificate fingerprint
  pinning, and a development-only `validate_certificate = false` mode.
* Configurable Exasol session initialization for the database-side SQL
  preprocessor.
* Simple Query and basic Extended Query support through `pgwire`.
* PostgreSQL-style row descriptions, data rows, command completions, errors, and
  ReadyForQuery handling.
* Extended Query execution sends row descriptions before result tuples so
  PostgreSQL clients do not receive `DataRow` messages without field metadata.
* Exasol-side PostgreSQL metadata compatibility for the documented PostgreSQL
  catalog and information-schema relation/column surface, with real mappings
  where Exasol has equivalent metadata and placeholders where PostgreSQL-only
  features have no Exasol equivalent.
* Read-only statement policy separated from Exasol execution.
* Local compatibility handling for common PostgreSQL client session commands and
  transaction wrappers.
* Multi-statement Simple Query splitting, with execution stopped after the first
  error response.
* Text prepared statement parameters for the Extended Query path.
* Explicit rejection for DML, DDL, binary prepared statement parameters, and
  multi-statement Extended Query Parse payloads.

SQL dialect translation remains inside Exasol through
`sql/exasol_sql_preprocessor.sql`; the gateway does not run `sqlglot` as an
external application.

## Changed Files

* `Cargo.toml`
* `Cargo.lock`
* `README.md`
* `.gitignore`
* `config/example.toml`
* `docs/smoke-test.md`
* `packaging/exa-postgres-interface.service`
* `src/config.rs`
* `src/exasol.rs`
* `src/main.rs`
* `src/metadata.rs`
* `src/pg_server.rs`
* `src/policy.rs`
* `tests/jdbc/PgJdbcSmoke.java`
* `tests/jdbc/PgJdbcMetaSmoke.java`
* `docs/postgres-metadata-compatibility.md`
* `specs/mission.md`
* `specs/_plans/add-read-only-postgres-interface/implementation-notes.md`
* `specs/_plans/add-read-only-postgres-interface/protocol/read-only-query-path/spec.md`

Removed the obsolete Python prototype package and Python unit tests so the
repository has one active implementation path.

## Tests and Commands Run

* `rustup component add rustfmt` installed the formatter component for the local
  Rust toolchain.
* `cargo fmt` passed.
* `cargo fmt --check` passed.
* `cargo test` passed: 16 tests.
* `cargo build --release` passed.
* `git diff --check` passed.
* `exapump sql --profile nc-personal-2 "SELECT 1"` passed and returned `1`.
* Downloaded PostgreSQL client packages without system install privileges,
  extracted `psql` under `/tmp/pgclient`, and used it for live smoke tests.
* Downloaded PostgreSQL JDBC driver `42.7.8`, extracted OpenJDK 21 packages
  without system install privileges, compiled `tests/jdbc/PgJdbcSmoke.java`,
  and used it for live extended-protocol smoke tests.
* Started the release gateway locally on `127.0.0.1:15433` with a temporary
  config targeting `3.66.165.192:8563`.
* `psql` through the gateway passed `SELECT 1` and returned `1`.
* `psql` through the gateway passed
  `SELECT d.datname AS table_cat FROM pg_catalog.pg_database d`, returning
  `exasol`.
* `psql` through the gateway passed the PostgreSQL-flavored sample query using
  `::` casts and `ILIKE`, returning the expected three sample rows.
* PostgreSQL JDBC through the gateway passed `PreparedStatement` execution for
  `SELECT 1`, reporting `OK columns=1 rows=1`.
* PostgreSQL JDBC through the gateway passed `PreparedStatement` execution for a
  `pg_catalog.pg_database` query, reporting `OK columns=1 rows=1`.
* PostgreSQL JDBC through the gateway passed `PreparedStatement` execution for
  the PostgreSQL-flavored sample query, reporting `OK columns=3 rows=3`.
* PostgreSQL JDBC metadata through the gateway passed
  `DatabaseMetaData.getCatalogs()`, `getSchemas()`, `getTables()`, and
  `getColumns()`, returning real Exasol-backed schema, table, and column
  metadata.
* DbVisualizer-profile metadata queries through the gateway passed:
  `select * from pg_tables where schemaname != 'pg_catalog'`,
  `select TABLE_NAME from INFORMATION_SCHEMA.TABLES ...`,
  `select COLUMN_NAME from INFORMATION_SCHEMA.COLUMNS ...`,
  `select * from pg_user`,
  `select * from pg_group`,
  `select * from pg_stat_activity`, and
  `select * from pg_locks`.

Earlier live database checks from this plan remain valid:

* `scripts/setup_sample_data.sh` passed with `nc-personal-2`.
* `exapump sql --profile nc-personal-2 < sql/exasol_sql_preprocessor.sql`
  passed, installing the database-side Python preprocessor.
* The canonical PostgreSQL-flavored demo query passed in Exasol after activating
  the database-side preprocessor in the same session.

## Spec Scenarios Covered

Covered by implementation and unit tests:

* Configuration loading and TLS policy defaults.
* Exasol DSN fingerprint and `nocertcheck` handling.
* Read/write/DDL/transaction statement classification.
* Local handling for safe PostgreSQL driver session commands.
* Local handling for common transaction wrappers.
* Simple Query batch splitting for PostgreSQL protocol compatibility.
* Exasol WebSocket `Pong("EXECUTING")` progress-frame handling.

Covered by build/manual verification:

* The Rust binary builds in release mode.
* The configured Exasol Personal profile `nc-personal-2` is reachable with TLS.
* The Exasol-side SQL preprocessor and sample data path were previously verified
  against the same profile.
* A real PostgreSQL client, `psql`, can connect through the gateway and execute
  the smoke and sample queries.
* The PostgreSQL JDBC driver can connect through the gateway using extended
  query protocol and receives field metadata before result tuples.
* Initial PostgreSQL database metadata probes map the single Exasol database
  concept to the PostgreSQL database name `exasol`.
* Schema and object metadata probes map Exasol metadata system tables into the
  PostgreSQL and DbVisualizer query shapes documented in
  `docs/postgres-metadata-compatibility.md`.

## Known Gaps and Follow-Up Work

* PostgreSQL system catalog semantics are not fully emulated even though the
  documented relation and column surface exists. Placeholder views and `NULL`
  columns remain intentional for unsupported PostgreSQL-only features.
* Binary prepared statement parameters are not implemented.
* Extended Query describe responses do not infer result schema before execution.
* Transaction wrappers are client compatibility acknowledgements, not
  Exasol-backed PostgreSQL transaction semantics.
* Query result values are currently returned as PostgreSQL text fields for broad
  client compatibility; richer row-value type mapping is future work.
