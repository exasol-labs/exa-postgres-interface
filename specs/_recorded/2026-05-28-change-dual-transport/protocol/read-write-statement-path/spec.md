# Feature: PostgreSQL Read/Write Statement Path

The protocol server SHALL route PostgreSQL client statements through a documented capability policy. Supported statements SHALL execute against Exasol or through explicit gateway-managed compatibility behavior, and unsupported statements SHALL fail clearly without leaving ambiguous session state.

Exasol execution SHALL occur through the transport selected by `exasol.transport`. The gateway-managed cursor registry SHALL hold materialised results in the shape native to the active transport (`RecordBatch` values for the Arrow transport, typed string rows for the WebSocket transport) and SHALL render either shape into PostgreSQL data rows on `FETCH` and `MOVE`.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* The protocol server opens one Exasol session per accepted client session through the configured transport.
* PostgreSQL clients observe result rows, command completion tags, affected-row counts, errors, and transaction status.
* Row counts surface as a single transport-agnostic shape: `ExasolOutcome::RowCount(i64)`.
* The gateway-managed cursor stores either Arrow `RecordBatch` values or typed string-row values, never both for the same cursor.

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Supported DML returns command completion

* *GIVEN* the compatibility matrix marks a PostgreSQL DML statement as supported
* *WHEN* the user sends `INSERT`, `UPDATE`, `DELETE`, or `MERGE` syntax that can be translated to Exasol
* *THEN* the server SHALL execute the translated statement against Exasol through the configured transport
* *AND* the server SHALL read the affected-row count from `ExasolOutcome::RowCount(i64)` returned by the transport
* *AND* the server SHALL return a PostgreSQL-compatible command completion response that carries the Exasol-reported row count when Exasol exposes one
* *AND* the server SHALL document any case where the PostgreSQL command tag cannot represent Exasol behavior exactly
* *AND* the observable client-side command completion SHALL be identical whether the active transport is `websocket` or `arrow`

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: search_path open failure surfaces as a PostgreSQL-compatible error

* *GIVEN* the client sends `SET search_path TO "Nonexistent"`
* *WHEN* Exasol rejects the `OPEN SCHEMA` statement executed through the configured transport
* *THEN* the server SHALL surface the failure as a PostgreSQL-compatible `ErrorResponse`
* *AND* the server SHALL leave the previously active schema unchanged in the session
* *AND* the server SHALL keep the client session usable for later supported statements
* *AND* the behavior SHALL be identical whether the active transport is `websocket` or `arrow`

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Cursors materialise results in the transport's native shape

* *GIVEN* the client has an active session through the protocol server
* *AND* the client issues `DECLARE <cursor> FOR <supported row-returning query>`
* *WHEN* the gateway executes the cursor query against Exasol through the configured transport
* *THEN* the gateway SHALL store the result inside the cursor registry in the shape returned by the active transport
* *AND* the Arrow transport SHALL store the result as Apache Arrow `RecordBatch` values
* *AND* the WebSocket transport SHALL store the result as typed columns plus string-row values
* *AND* the gateway SHALL serve subsequent `FETCH` and `MOVE` commands from the stored cursor data without re-executing the query
* *AND* the gateway SHALL encode fetched rows for PostgreSQL using the per-transport renderer documented for ad-hoc queries

<!-- /DELTA:CHANGED -->
