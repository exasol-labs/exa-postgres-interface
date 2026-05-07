# Feature: PostgreSQL-To-Exasol Compatibility Matrix

The project SHALL maintain a PostgreSQL-to-Exasol compatibility matrix that maps PostgreSQL syntax, functions, data types, commands, metadata, and session behavior to Exasol equivalents, gateway-managed behavior, or explicit unsupported status. The matrix is the source of truth for broadening the gateway from read-only queries to read/write compatibility.

## Background

* PostgreSQL 18 documents the current SQL command catalog used as the source syntax inventory.
* Exasol documentation is the source of truth for supported Exasol statements, functions, data types, SQL standard features, and documented limitations.
* Exasol supports core read, write, DDL, privilege, and transaction statements, but its syntax and behavior are not identical to PostgreSQL.
* Translation failures, unsupported PostgreSQL features, and Exasol execution failures are separate outcomes.
* The preferred translation owner is the gateway application layer, not an Exasol-side SQL preprocessor.
* PostgreSQL session-state commands (`SET`, `SHOW`, `RESET`) MAY require gateway-managed compatibility when they map to Exasol session-state verbs (such as `OPEN SCHEMA`) rather than to direct SQL.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: search_path session command maps to Exasol OPEN SCHEMA

* *GIVEN* the project documents PostgreSQL session command compatibility
* *WHEN* the matrix records support for `SET search_path`
* *THEN* the matrix SHALL classify single-schema `SET search_path = <schema>` as `gateway-managed` with Exasol `OPEN SCHEMA <schema>` as the equivalent statement
* *AND* the matrix SHALL classify multi-schema `SET search_path` as `unsupported-no-equivalent` because Exasol does not support multiple simultaneously active schemas
* *AND* the matrix SHALL classify `RESET search_path` and `SET search_path = DEFAULT` as `gateway-managed` no-ops because Exasol has no documented "close schema" command
* *AND* the matrix SHALL document the gateway response shape including the PostgreSQL command completion tag returned to the client
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: SHOW search_path reads gateway-managed session state

* *GIVEN* the project documents PostgreSQL `SHOW` compatibility
* *WHEN* the matrix records support for `SHOW search_path`
* *THEN* the matrix SHALL classify `SHOW search_path` as `gateway-managed` whose value is the gateway-tracked active schema after a successful `SET search_path`
* *AND* the matrix SHALL state the documented default value returned when no schema has been opened during the session
* *AND* the matrix SHALL state that the gateway does NOT proxy `SHOW search_path` to an Exasol statement
<!-- /DELTA:NEW -->
