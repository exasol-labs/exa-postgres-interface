# Feature: PostgreSQL Read/Write Statement Path

The protocol server SHALL route PostgreSQL client statements through a documented capability policy instead of a read-only policy. Supported statements SHALL execute against Exasol or through explicit gateway-managed compatibility behavior, and unsupported statements SHALL fail clearly without leaving ambiguous session state.

Exasol execution SHALL occur through the `exarrow-rs` async driver. Arrow `RecordBatch` results and Exasol-reported row counts SHALL be the only result shapes the gateway carries between transport and PostgreSQL response mapping.

PostgreSQL wire compatibility remains a client integration layer. Exasol remains the database of record.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* The protocol server opens one Exasol session per accepted client session through the `exarrow-rs` driver.
* PostgreSQL clients observe result rows, command completion tags, affected-row counts, errors, and transaction status.
* Result-returning Exasol statements expose data as Apache Arrow `RecordBatch` values; row-modifying statements expose an Exasol row count.
* The gateway-managed cursor registry holds materialised Arrow `RecordBatch` data, not pre-stringified rows.
* Exasol supports core read and write SQL families, but not every PostgreSQL command or object model.
* Some client-visible PostgreSQL behaviors, including SQL cursors, may need gateway-managed compatibility.
* PostgreSQL clients use `SET search_path` and `SHOW search_path` to scope or inspect the active schema; Exasol exposes equivalent state through `OPEN SCHEMA` and `current_schema()`.

## Scenarios

### Scenario: Statement routing uses a capability matrix

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the user sends a PostgreSQL statement
* *THEN* the server SHALL classify the statement using the documented compatibility matrix
* *AND* the server SHALL route supported statements to Exasol execution or gateway-managed compatibility behavior
* *AND* the server SHALL reject unsupported statements with a PostgreSQL-compatible error that identifies the unsupported capability


### Scenario: Supported DML returns command completion

* *GIVEN* the compatibility matrix marks a PostgreSQL DML statement as supported
* *WHEN* the user sends `INSERT`, `UPDATE`, `DELETE`, or `MERGE` syntax that can be translated to Exasol
* *THEN* the server SHALL execute the translated statement against Exasol through `exarrow-rs`
* *AND* the server SHALL read the affected-row count from the Exasol row-count result returned by the driver
* *AND* the server SHALL return a PostgreSQL-compatible command completion response that carries the Exasol-reported row count when Exasol exposes one
* *AND* the server SHALL document any case where the PostgreSQL command tag cannot represent Exasol behavior exactly


### Scenario: Supported DDL is capability-scoped

* *GIVEN* the compatibility matrix marks a PostgreSQL DDL statement as supported
* *WHEN* the user sends DDL for an Exasol-equivalent object type
* *THEN* the server SHALL translate and execute the statement against Exasol
* *AND* the server SHALL return a PostgreSQL-compatible command completion response
* *AND* the server SHALL document semantic differences such as object replacement, constraints, identity columns, distribution keys, partitioning, privileges, and unsupported object attributes


### Scenario: PostgreSQL-only objects are rejected

* *GIVEN* the client sends valid PostgreSQL syntax for a PostgreSQL-specific object with no Exasol equivalent
* *WHEN* the statement is classified as `unsupported-no-equivalent`
* *THEN* the server SHALL reject the statement with a PostgreSQL-compatible error
* *AND* the server SHALL NOT create placeholder Exasol objects that imply unsupported PostgreSQL semantics
* *AND* the server SHALL keep the client session usable when Exasol session state remains valid


### Scenario: Transaction commands expose Exasol-backed behavior

* *GIVEN* the client sends transaction-related commands such as `BEGIN`, `COMMIT`, `ROLLBACK`, or `END`
* *WHEN* the command is supported by the compatibility matrix
* *THEN* the server SHALL map the command to Exasol transaction behavior or a documented local protocol behavior
* *AND* the server SHALL report PostgreSQL ReadyForQuery transaction status consistently with the documented behavior
* *AND* the server SHALL NOT claim PostgreSQL savepoint, two-phase commit, or isolation semantics unless those semantics are implemented and verified against Exasol


### Scenario: Failed write statements recover predictably

* *GIVEN* the client sends a supported write statement
* *WHEN* translation or Exasol execution fails
* *THEN* the server SHALL return a PostgreSQL-compatible error
* *AND* the server SHALL log whether the failure occurred during policy classification, translation, Exasol execution, or protocol response mapping
* *AND* the server SHALL make the resulting transaction/session state observable through ReadyForQuery status and logs
