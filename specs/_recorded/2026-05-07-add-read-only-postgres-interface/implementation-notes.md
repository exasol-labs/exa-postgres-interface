# Implementation Notes

Status as of 2026-04-27: the Rust/`pgwire` implementation remains the active
gateway. PostgreSQL metadata compatibility has since moved into Exasol-side
`PG_CATALOG` and `INFORMATION_SCHEMA` schemas, with
`PG_DEMO.PG_SQL_PREPROCESSOR` handling PostgreSQL dialect and metadata-query
rewrites inside Exasol.

## Runtime Selection

The active prototype uses Rust 1.89 and the `pgwire` crate.

Reasons:

* The initial direct Python wire-protocol prototype was not stable enough for
  DbVisualizer's connection behavior.
* `pgwire` provides startup/authentication, Simple Query, Extended Query,
  response framing, connection state, and ReadyForQuery handling as protocol
  primitives instead of ad hoc socket message construction.
* Rust produces a standalone Linux binary that matches the preferred operational
  shape for a systemd-managed gateway.
* SQL dialect translation remains inside Exasol through the configured Python
  preprocessor script. The gateway does not run `sqlglot` as an external
  application.

Tradeoffs:

* PostgreSQL system catalog compatibility is now broad enough to expose the
  documented PostgreSQL 18 relation and column surface, but PostgreSQL-only
  engine features still return empty or `NULL` compatibility metadata.
* Prepared statement parameter support is intentionally limited; binary
  prepared statement parameters remain unsupported.
* Common transaction wrappers are acknowledged locally for client compatibility,
  but real Exasol-backed transaction semantics are not implemented.

## Architecture

The implementation separates:

* Configuration loading in `src/config.rs`.
* Exasol WebSocket session handling, TLS policy, certificate fingerprint
  verification, login, execution, and result parsing in `src/exasol.rs`.
* PostgreSQL startup/authentication, Simple Query, Extended Query, response
  encoding, and per-client Exasol session state in `src/pg_server.rs`.
* Statement classification and local client compatibility commands in
  `src/policy.rs`.
* Process startup and listener lifecycle in `src/main.rs`.

The execution result model distinguishes row-returning query results from
command-completion results so future write support can add DML/DDL behavior
without replacing the protocol response path.

Operationally, the preferred deployment shape is now a release binary installed
under `/opt/exa-postgres-interface/bin`, a TOML config under
`/etc/exa-postgres-interface`, and the provided systemd unit in
`packaging/exa-postgres-interface.service`.

## PostgreSQL Protocol Reference

The implementation follows the PostgreSQL frontend/backend protocol message
flow for the currently supported paths:

* Startup requests cleartext password authentication, then completes with
  AuthenticationOk, ParameterStatus, BackendKeyData, and ReadyForQuery.
* Simple Query sends one or more response sequences and ends with ReadyForQuery.
* Empty simple queries return EmptyQueryResponse.
* Extended Query accepts Parse/Bind/Describe/Execute/Sync, rejects
  multi-statement Parse payloads, and rejects bound parameters until parameter
  translation is implemented.
