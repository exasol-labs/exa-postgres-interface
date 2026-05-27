# Feature: PostgreSQL Read/Write Statement Path

The protocol server SHALL route PostgreSQL client statements through a documented capability policy. Supported statements SHALL execute against Exasol or through explicit gateway-managed compatibility behavior, and unsupported statements SHALL fail clearly without leaving ambiguous session state.

Exasol execution SHALL occur through the `exarrow-rs` async driver. Arrow `RecordBatch` results and Exasol-reported row counts SHALL be the only result shapes the gateway carries between transport and PostgreSQL response mapping.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* The protocol server opens one Exasol session per accepted client session through the `exarrow-rs` driver.
* PostgreSQL clients observe result rows, command completion tags, affected-row counts, errors, and transaction status.
* Result-returning Exasol statements expose data as Apache Arrow `RecordBatch` values; row-modifying statements expose an Exasol row count.
* The gateway-managed cursor registry holds materialised Arrow `RecordBatch` data, not pre-stringified rows.

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Supported DML returns command completion

* *GIVEN* the compatibility matrix marks a PostgreSQL DML statement as supported
* *WHEN* the user sends `INSERT`, `UPDATE`, `DELETE`, or `MERGE` syntax that can be translated to Exasol
* *THEN* the server SHALL execute the translated statement against Exasol through `exarrow-rs`
* *AND* the server SHALL read the affected-row count from the Exasol row-count result returned by the driver
* *AND* the server SHALL return a PostgreSQL-compatible command completion response that carries the Exasol-reported row count when Exasol exposes one
* *AND* the server SHALL document any case where the PostgreSQL command tag cannot represent Exasol behavior exactly

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: search_path open failure surfaces as a PostgreSQL-compatible error

* *GIVEN* the client sends `SET search_path TO "Nonexistent"`
* *WHEN* Exasol rejects the `OPEN SCHEMA` statement executed through `exarrow-rs`
* *THEN* the server SHALL surface the failure as a PostgreSQL-compatible `ErrorResponse`
* *AND* the server SHALL leave the previously active schema unchanged in the session
* *AND* the server SHALL keep the client session usable for later supported statements

<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: Cursors materialise Arrow record batches from Exasol

* *GIVEN* the client has an active session through the protocol server
* *AND* the client issues `DECLARE <cursor> FOR <supported row-returning query>`
* *WHEN* the gateway executes the cursor query against Exasol through `exarrow-rs`
* *THEN* the gateway SHALL store the result inside the cursor registry as Apache Arrow `RecordBatch` values
* *AND* the gateway SHALL serve subsequent `FETCH` and `MOVE` commands from the stored `RecordBatch` values without re-executing the query
* *AND* the gateway SHALL encode fetched rows for PostgreSQL using the same Arrow-to-PostgreSQL rendering as ad-hoc queries

<!-- /DELTA:NEW -->
