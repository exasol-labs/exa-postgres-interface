# Feature: Read-Only PostgreSQL Query Path

Status as of 2026-04-27: implemented for the current read-only capability
scope. Simple Query and Extended Query paths work for row-returning statements,
common session commands, transaction wrappers, JDBC metadata probes, and the
catalog browser paths exercised so far.

Future target scope note: this read-only feature spec describes the implemented
prototype baseline. The next design direction is defined in
`specs/_plans/change-postgres-read-write-compatibility/` and replaces the
read-only policy with a capability-based read/write compatibility model.

The protocol server SHALL provide the smallest PostgreSQL-compatible connection and query path needed for DbVisualizer to reach Exasol. The server SHALL preserve Exasol as the executing database and SHALL make unsupported PostgreSQL behavior explicit.

The first supported statement scope is read-only DQL, but the protocol server SHOULD be designed as a session-oriented gateway that can add write-capable PostgreSQL behavior later without replacing the connection, authentication, session, or response-mapping architecture.

PostgreSQL wire compatibility SHALL be treated as a client integration layer over Exasol execution. The server SHALL NOT imply full PostgreSQL SQL semantics unless a behavior has been explicitly implemented, translated, and documented.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* DbVisualizer is the first required client.
* The protocol server opens an Exasol session for each accepted client session.
* The first query scope is read-only DQL.
* Future versions may support DML, DDL, transaction behavior, prepared statements, and richer metadata behavior.
* PostgreSQL-compatible clients observe command completion tags, affected-row counts, errors, and transaction state in addition to result rows.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: DbVisualizer connects through the PostgreSQL connector

* *GIVEN* the protocol server is listening for PostgreSQL wire-protocol connections
* *AND* DbVisualizer is configured to use its PostgreSQL connector against the protocol server
* *WHEN* the user opens the connection
* *THEN* the server SHALL complete the minimum PostgreSQL startup exchange required by DbVisualizer
* *AND* the server SHALL authenticate using username and password credentials supplied by the client
* *AND* the server SHALL open a corresponding Exasol session for the client connection

<!-- DELTA:NEW -->
### Scenario: Client credentials are passed to Exasol

* *GIVEN* the PostgreSQL client supplies a username and password during connection startup
* *WHEN* the protocol server creates the Exasol session
* *THEN* the server SHOULD use the client-supplied username and password to authenticate to Exasol
* *AND* the server SHALL fail the client connection with a clear PostgreSQL-compatible error if Exasol rejects the credentials

<!-- DELTA:NEW -->
### Scenario: User runs the simplest smoke-test query

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the user runs `SELECT 1`
* *THEN* the server SHALL execute the query against Exasol
* *AND* the server SHALL return a PostgreSQL-compatible row description, data row, command completion, and ready state
* *AND* the result SHALL be visible to the client as a single row containing the value `1`

<!-- DELTA:NEW -->
### Scenario: User runs a read-only query against sample data

* *GIVEN* the client has an active session through the protocol server
* *AND* sample data exists in Exasol
* *WHEN* the user runs a supported read-only DQL query
* *THEN* the server SHALL execute the query against Exasol
* *AND* the server SHALL return tabular results in a form the PostgreSQL client can consume

<!-- DELTA:NEW -->
### Scenario: Unsupported behavior returns a warning and client-visible error

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the client sends unsupported protocol behavior, unsupported SQL, or unsupported metadata behavior
* *THEN* the server SHALL log a warning that identifies the unsupported behavior
* *AND* the server SHALL return a clear PostgreSQL-compatible error to the client
* *AND* the server SHALL NOT silently emulate behavior that changes meaningful Exasol semantics

<!-- DELTA:NEW -->
### Scenario: Write statements are rejected in the first prototype scope

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the user sends DDL, DML, or another command outside the read-only DQL scope
* *THEN* the server SHALL reject the statement with a PostgreSQL-compatible error
* *AND* the server SHALL log a warning that the command is outside the prototype scope
* *AND* the rejection SHALL be implemented as an explicit capability policy so future write support can replace the rejection without replacing the connection/session architecture

<!-- DELTA:NEW -->
### Scenario: Statement handling remains extensible for future write support

* *GIVEN* the first prototype only enables read-only DQL execution
* *WHEN* the server classifies and routes a client statement
* *THEN* the server SHOULD keep statement classification separate from Exasol execution
* *AND* the server SHOULD keep protocol response mapping extensible for future command completion responses, update counts, transaction state changes, and write-related errors
* *AND* the server SHALL NOT rely on assumptions that every successful statement returns a result set

<!-- DELTA:NEW -->
### Scenario: Rejected statements do not poison the client session

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the user sends a write statement that is rejected by the first prototype policy
* *THEN* the server SHALL return a PostgreSQL-compatible error for the rejected statement
* *AND* the server SHOULD keep the client session usable for later supported read-only DQL statements when Exasol session state remains valid

<!-- DELTA:NEW -->
### Scenario: Future non-row-returning statements have a response model

* *GIVEN* a future supported statement changes data or schema without returning rows
* *WHEN* the server executes the statement against Exasol
* *THEN* the server SHALL be able to return a PostgreSQL-compatible command completion response
* *AND* the server SHOULD include affected-row counts when Exasol exposes reliable affected-row information
* *AND* the server SHALL document cases where PostgreSQL command tags or counts cannot represent Exasol behavior exactly

<!-- DELTA:NEW -->
### Scenario: Transaction compatibility is explicit

* *GIVEN* a PostgreSQL client sends transaction-related commands such as `BEGIN`, `COMMIT`, or `ROLLBACK`
* *WHEN* transaction behavior is not implemented for the current capability scope
* *THEN* the server SHALL either reject the command with a clear PostgreSQL-compatible error or implement documented client-compatibility behavior
* *AND* any local acknowledgement SHALL be documented as protocol compatibility rather than Exasol transaction semantics
* *AND* the server SHALL NOT claim full PostgreSQL transaction semantics until those semantics are implemented against Exasol
