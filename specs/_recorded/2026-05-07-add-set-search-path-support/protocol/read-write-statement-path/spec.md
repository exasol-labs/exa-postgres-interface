# Feature: PostgreSQL Read/Write Statement Path

The protocol server SHALL route PostgreSQL client statements through a documented capability policy instead of a read-only policy. Supported statements SHALL execute against Exasol or through explicit gateway-managed compatibility behavior, and unsupported statements SHALL fail clearly without leaving ambiguous session state.

PostgreSQL wire compatibility remains a client integration layer. Exasol remains the database of record.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* The protocol server opens one Exasol session per accepted client session.
* PostgreSQL clients observe result rows, command completion tags, affected-row counts, errors, and transaction status.
* Exasol supports core read and write SQL families, but not every PostgreSQL command or object model.
* Some client-visible PostgreSQL behaviors, including SQL cursors, may need gateway-managed compatibility.
* PostgreSQL clients use `SET search_path` and `SHOW search_path` to scope or inspect the active schema; Exasol exposes equivalent state through `OPEN SCHEMA` and `current_schema()`.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Single-schema search_path opens the Exasol schema

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `SET search_path = <schema>` or `SET search_path TO <schema>` with exactly one schema identifier
* *THEN* the server SHALL classify the statement as a gateway-managed search_path assignment
* *AND* the server SHALL execute `OPEN SCHEMA <schema>` against Exasol using the active session
* *AND* the server SHALL update gateway-managed session state so that the active schema reflects the new value
* *AND* the server SHALL return a PostgreSQL-compatible `SET` command completion response
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Multi-schema search_path is rejected with a compatibility error

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `SET search_path` with more than one comma-separated schema identifier
* *THEN* the server SHALL reject the statement with a PostgreSQL-compatible error stating that only single-schema search paths are supported
* *AND* the server SHALL NOT change gateway-managed session state or issue any `OPEN SCHEMA` against Exasol
* *AND* the server SHALL keep the client session usable for subsequent statements
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: search_path reset is a no-op

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `RESET search_path`, `SET search_path = DEFAULT`, or `SET search_path TO DEFAULT`
* *THEN* the server SHALL accept the statement and return a PostgreSQL-compatible command completion response
* *AND* the server SHALL NOT issue any Exasol statement on behalf of the request
* *AND* the server SHALL leave gateway-managed session state unchanged because Exasol has no documented "close schema" command
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: SHOW search_path reflects the active schema

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `SHOW search_path`
* *THEN* the server SHALL return the gateway-tracked active schema when one has been set during the session
* *AND* the server SHALL return the documented default value when no schema has been opened during the session
* *AND* the response column name SHALL match the PostgreSQL `SHOW search_path` convention
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: search_path open failure surfaces as a PostgreSQL-compatible error

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `SET search_path = <schema>` and Exasol rejects the resulting `OPEN SCHEMA <schema>` statement
* *THEN* the server SHALL return a PostgreSQL-compatible error that identifies the failure
* *AND* the server SHALL NOT update gateway-managed session state
* *AND* the server SHALL keep the client session usable for subsequent statements
<!-- /DELTA:NEW -->
