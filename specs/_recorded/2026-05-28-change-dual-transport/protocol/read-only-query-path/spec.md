# Feature: Read-Only PostgreSQL Query Path

The protocol server SHALL provide the smallest PostgreSQL-compatible connection and query path needed for DbVisualizer to reach Exasol. The server SHALL preserve Exasol as the executing database and SHALL make unsupported PostgreSQL behavior explicit.

The Exasol session SHALL communicate with Exasol through a configurable transport. The gateway SHALL carry result data in the shape native to the active transport: Apache Arrow `RecordBatch` values when the Arrow transport is active, and typed string-row results (with Exasol JSON-supplied column metadata) when the WebSocket transport is active. The wire-protocol mapping into PostgreSQL rows SHALL be defined for both shapes.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* The protocol server opens an Exasol session for each accepted client session.
* The Exasol session is provided by one of two transports — the WebSocket JSON transport or the `exarrow-rs` Apache Arrow transport — selected at startup by `exasol.transport`.
* Both transports run on the same Tokio runtime as the PostgreSQL wire-protocol server.
* Both transports expose a uniform asynchronous session interface (`ExasolTransport`) inside the gateway, returning a transport-tagged outcome (`ExasolOutcome::ArrowRows`, `ExasolOutcome::TypedRows`, or `ExasolOutcome::RowCount`).

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Client credentials are passed to Exasol

* *GIVEN* the PostgreSQL client supplies a username and password during connection startup
* *WHEN* the protocol server creates the Exasol session through the configured transport
* *THEN* the server SHALL use the client-supplied username and password to authenticate to Exasol
* *AND* the server SHALL fail the client connection with a clear PostgreSQL-compatible error if Exasol rejects the credentials
* *AND* the server SHALL NOT block a Tokio worker thread while waiting on Exasol authentication
* *AND* the server SHALL authenticate identically whether the active transport is `websocket` or `arrow`

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: User runs the simplest smoke-test query

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the user runs `SELECT 1`
* *THEN* the server SHALL execute the query against Exasol through the configured transport
* *AND* the server SHALL render the transport's result into a PostgreSQL-compatible row description, data row, command completion, and ready state
* *AND* the result SHALL be visible to the client as a single row containing the value `1`
* *AND* the observable client-side output SHALL be identical whether the active transport is `websocket` or `arrow`

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Result values traverse the gateway in the transport's native shape

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the server executes any row-returning statement against Exasol
* *THEN* the server SHALL hold the result inside the gateway in the shape returned by the active transport without an intermediate re-encoding
* *AND* the Arrow transport SHALL produce Apache Arrow `RecordBatch` values
* *AND* the WebSocket transport SHALL produce typed string-row results carrying Exasol's JSON `dataType` metadata per column
* *AND* the server SHALL render each shape into PostgreSQL fields using a documented per-transport type mapping
* *AND* the server SHALL encode NULL values as PostgreSQL NULLs in the data row for both shapes

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Exasol session calls are awaited on the Tokio runtime

* *GIVEN* the gateway accepts a PostgreSQL client connection
* *WHEN* the gateway opens an Exasol session, runs any session-initialization SQL, or executes a client statement
* *THEN* the gateway SHALL drive each transport call through `async`/`await` on the existing Tokio runtime
* *AND* the gateway MUST NOT wrap Exasol calls in `task::spawn_blocking` or `block_in_place`
* *AND* the gateway SHALL guard the shared Exasol session with `tokio::sync::Mutex` so concurrent client requests serialize without poisoning a synchronous lock
* *AND* the requirement SHALL hold for both the `websocket` and `arrow` transports

<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: WebSocket transport returns typed string rows with Exasol type OIDs

* *GIVEN* the gateway is configured with `exasol.transport = "websocket"`
* *AND* the client has an active session through the protocol server
* *WHEN* the server executes a row-returning statement against Exasol
* *THEN* the WebSocket transport SHALL parse Exasol's JSON result envelope into typed columns and string-row values
* *AND* the gateway SHALL map each column's Exasol `dataType` metadata to a PostgreSQL type OID using the documented JSON-based OID mapping
* *AND* the gateway SHALL emit each row to the client as PostgreSQL data rows whose field bytes are the string values returned by Exasol
* *AND* the gateway SHALL encode JSON `null` column values as PostgreSQL NULLs

<!-- /DELTA:NEW -->
