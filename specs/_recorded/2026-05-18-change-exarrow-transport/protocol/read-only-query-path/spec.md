# Feature: Read-Only PostgreSQL Query Path

The protocol server SHALL provide the smallest PostgreSQL-compatible connection and query path needed for DbVisualizer to reach Exasol. The server SHALL preserve Exasol as the executing database and SHALL make unsupported PostgreSQL behavior explicit.

The Exasol session SHALL communicate with Exasol through the `exarrow-rs` async driver. Result data SHALL be carried inside the gateway as Apache Arrow `RecordBatch` values until it is rendered into the PostgreSQL wire protocol response.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* The protocol server opens an Exasol session for each accepted client session.
* The Exasol session is provided by the `exarrow-rs` async driver and runs on the same Tokio runtime as the PostgreSQL wire-protocol server.
* Exasol query results MUST cross the gateway as Apache Arrow `RecordBatch` values, not as pre-stringified rows.

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Client credentials are passed to Exasol

* *GIVEN* the PostgreSQL client supplies a username and password during connection startup
* *WHEN* the protocol server creates the Exasol session through the `exarrow-rs` driver
* *THEN* the server SHALL use the client-supplied username and password to authenticate to Exasol
* *AND* the server SHALL fail the client connection with a clear PostgreSQL-compatible error if Exasol rejects the credentials
* *AND* the server SHALL NOT block a Tokio worker thread while waiting on Exasol authentication

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: User runs the simplest smoke-test query

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the user runs `SELECT 1`
* *THEN* the server SHALL execute the query against Exasol through the `exarrow-rs` driver
* *AND* the Exasol driver SHALL return the result as one or more Apache Arrow `RecordBatch` values
* *AND* the server SHALL render the Arrow result into a PostgreSQL-compatible row description, data row, command completion, and ready state
* *AND* the result SHALL be visible to the client as a single row containing the value `1`

<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: Result values traverse the gateway as Apache Arrow record batches

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the server executes any row-returning statement against Exasol
* *THEN* the server SHALL hold the result inside the gateway as Apache Arrow `RecordBatch` values
* *AND* the server SHALL render each Arrow column into a PostgreSQL field using a documented Arrow-to-PostgreSQL type mapping
* *AND* the server SHALL encode NULL Arrow values as PostgreSQL NULLs in the data row
* *AND* the server SHALL NOT introduce a pre-stringified row representation as an intermediate gateway data structure

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Exasol session calls are awaited on the Tokio runtime

* *GIVEN* the gateway accepts a PostgreSQL client connection
* *WHEN* the gateway opens an Exasol session, runs any session-initialization SQL, or executes a client statement
* *THEN* the gateway SHALL drive each `exarrow-rs` call through `async`/`await` on the existing Tokio runtime
* *AND* the gateway MUST NOT wrap Exasol calls in `task::spawn_blocking` or `block_in_place`
* *AND* the gateway SHALL guard the shared Exasol session with `tokio::sync::Mutex` so concurrent client requests serialize without poisoning a synchronous lock

<!-- /DELTA:NEW -->
