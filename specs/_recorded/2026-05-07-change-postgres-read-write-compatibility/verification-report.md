# Verification Report: Change PostgreSQL Read/Write Compatibility Scope

## Summary

Implemented the first gateway slice of the read/write compatibility plan. The gateway no longer treats statement routing as a read-only policy. It now classifies statements through explicit capabilities and routes supported statements to Exasol execution.

This implementation does not complete the full compatibility matrix. It enables core Exasol-backed DML and selected table/view/schema DDL, improves command completion tags for row-count statements, and keeps higher-risk PostgreSQL-only or not-yet-designed behavior explicitly unsupported.

## Changed Files

* `src/policy.rs`
  * Replaced the `Read` statement plan with `Execute { command, row_count }`.
  * Added row-count policy for command completion tags.
  * Enabled Exasol-backed execution for `SELECT`, `WITH`, `VALUES`, `INSERT`, `UPDATE`, `DELETE`, `MERGE`, `TRUNCATE`, table/view/schema `CREATE`, `ALTER`, `DROP`, `COMMENT`, `GRANT`, and `REVOKE`.
  * Added SQL cursor classification for supported `DECLARE`, `FETCH`, `MOVE`, and `CLOSE` forms.
  * Rejected PostgreSQL-only or not-yet-designed behavior with explicit unsupported-by-policy or no-equivalent messages.
  * Kept SQL-level prepared statement commands, bulk `COPY`, savepoints, PostgreSQL maintenance commands, async notifications, roles/users, binary cursors, updatable cursors, and routine mapping unsupported for now.
  * Updated local `SHOW transaction_read_only` compatibility response to `off`.
* `src/pg_server.rs`
  * Updated startup server parameters so clients no longer see the gateway as default read-only.
  * Passed statement command metadata into Exasol result mapping.
  * Returned DML row counts with the originating command tag instead of `OK`.
  * Omitted row counts for selected DDL command tags.
  * Added per-session gateway-managed cursor state.
  * Added materialized read-only cursor declaration, fetching, movement, close, duplicate-name checks, and non-hold cleanup at transaction end.
  * Added `FETCH <count>` command-tag support for cursor row responses.
* `README.md`
  * Updated status and known limits from read-only to capability-based routing.
* `specs/_plans/change-postgres-read-write-compatibility/verification-report.md`
  * Added this report.

## Spec Scenarios Covered

Covered:

* `protocol/read-write-statement-path`: Statement routing uses a capability matrix.
* `protocol/read-write-statement-path`: Supported DML returns command completion.
* `protocol/read-write-statement-path`: Supported DDL is capability-scoped.
* `protocol/read-write-statement-path`: PostgreSQL-only objects are rejected.
* `protocol/read-write-statement-path`: Prepared statements and protocol portals are designed with write support, for the existing extended-query path.
* `protocol/read-write-statement-path`: SQL cursor declaration is gateway-managed.
* `protocol/read-write-statement-path`: SQL cursor fetch returns result rows with `FETCH <count>` command completion.
* `protocol/read-write-statement-path`: SQL cursor movement and cleanup are explicit.
* `protocol/read-write-statement-path`: Unsupported cursor semantics fail safely.
* `sql/postgres-to-exasol-compatibility`: Unsupported by policy is distinct from unsupported by Exasol.

Partially covered:

* `protocol/read-write-statement-path`: Transaction commands expose Exasol-backed behavior. Transaction wrappers remain local compatibility behavior; full Exasol-backed transaction semantics are still not implemented.
* `protocol/read-write-statement-path`: Cursor holdability is represented in session state and non-hold cursors are cleared on transaction end, but full PostgreSQL transaction semantics are still local compatibility behavior.
* `sql/postgres-to-exasol-compatibility`: Core DML is mapped before broader write behavior. The first DML pass is enabled, but live Exasol integration verification is still needed.

Not covered yet:

* Bulk `COPY` design.
* Full command-level compatibility matrix.
* SQLGlot versus Polyglot translator evaluation.
* Direct Exasol integration verification for each newly enabled write/DDL family.
* Cursor memory limits, spill-to-disk policy, binary cursor output, updatable cursors, and positioned cursor writes.

## Tests And Commands Run

* `cargo fmt`
* `cargo fmt --check`
* `cargo test --bin exa-postgres-interface cursor`
* `cargo test --bin exa-postgres-interface policy`
* `cargo test --bin exa-postgres-interface`
* `cargo test`

## Known Gaps

* SQL cursors are materialized in gateway memory at declaration time. They still require explicit memory limits, spill policy, timeout policy, and live Exasol integration verification.
* Binary cursors, updatable cursors, `FOR UPDATE`, `FOR SHARE`, `WHERE CURRENT OF`, and positioned writes remain unsupported.
* `COPY`, savepoints, two-phase commit, PostgreSQL async notification commands, maintenance commands, SQL-level prepared statement commands, roles/users, and routine DDL remain unsupported.
* The gateway still depends on the existing Exasol-side SQLGlot preprocessor. Polyglot has not been integrated or benchmarked.
* DML/DDL syntax compatibility depends on the current preprocessor and Exasol parser. Live Exasol integration tests should be added for successful `INSERT`, `UPDATE`, `DELETE`, `MERGE`, `TRUNCATE`, table/view/schema DDL, and unsupported PostgreSQL-only commands.
