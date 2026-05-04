# Feature: PostgreSQL-to-Exasol Read-Only SQL Translation

Status as of 2026-04-27: implemented through the Exasol-side
`PG_DEMO.PG_SQL_PREPROCESSOR`. The preprocessor now handles both general
PostgreSQL-to-Exasol dialect translation and targeted metadata-query rewrites
for observed DbVisualizer and DBeaver catalog browser behavior.

Future target scope note: this feature spec describes the implemented
read-only translation baseline. The next design direction is defined in
`specs/_plans/change-postgres-read-write-compatibility/` and broadens
translation requirements to capability-based read/write PostgreSQL
compatibility.

Read-only SQL sent by PostgreSQL-compatible clients SHOULD be converted from PostgreSQL dialect to Exasol dialect through an Exasol-side Python preprocessor based on `sqlglot`. Translation behavior SHALL be observable enough to debug failed conversion or execution.

The first translation scope is read-only DQL, but the translation boundary SHOULD allow future DML or DDL translation rules to be added intentionally instead of requiring a different preprocessing mechanism.

## Background

* Exasol remains the executing database.
* The SQL translation mechanism is an Exasol-side Python preprocessor.
* The preprocessor is expected to use `sqlglot`.
* Initial query scope is read-only DQL.
* Future write support may require different translation safety rules for DML, DDL, transaction statements, and update counts.
* Unsupported-by-policy statements are different from statements that fail translation or fail in Exasol.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Session initialization enables SQL translation

* *GIVEN* the protocol server has opened an Exasol session for a client connection
* *WHEN* the server initializes the session
* *THEN* the server SHALL activate or select the configured Python SQL preprocessor for that session
* *AND* the server SHALL log enough information to confirm that SQL translation is active
* *AND* the server SHALL fail the client connection with a clear PostgreSQL-compatible error if translation setup cannot complete

<!-- DELTA:NEW -->
### Scenario: PostgreSQL-flavored read-only query is translated before Exasol execution

* *GIVEN* the client has an initialized session with SQL translation active
* *WHEN* the user submits a supported PostgreSQL-flavored read-only DQL query
* *THEN* the preprocessor SHOULD convert the query to Exasol-compatible SQL before execution
* *AND* Exasol SHALL execute the converted query as the source of truth
* *AND* the protocol server SHALL return the Exasol result to the PostgreSQL client

<!-- DELTA:NEW -->
### Scenario: Demo query exercises dialect conversion

* *GIVEN* sample data exists for the prototype demo
* *WHEN* the user runs the canonical PostgreSQL-to-Exasol conversion demo query from the plan
* *THEN* the query SHOULD include PostgreSQL-specific `::` casts and `ILIKE` case-insensitive matching
* *AND* the converted query SHALL return deterministic tabular results from Exasol
* *AND* the demo documentation SHALL include both the user-facing PostgreSQL query and the expected result shape

<!-- DELTA:NEW -->
### Scenario: Translation failure is explicit

* *GIVEN* the client submits a read-only query that cannot be translated safely
* *WHEN* the preprocessor or server detects the translation failure
* *THEN* the server SHALL return a clear PostgreSQL-compatible error to the client
* *AND* the server SHALL log a warning with enough detail to identify the failed conversion
* *AND* the server SHALL NOT execute a partially converted query when conversion safety is unknown

<!-- DELTA:NEW -->
### Scenario: Translation boundary allows future write support

* *GIVEN* the first prototype only permits read-only DQL statements
* *WHEN* the server or preprocessor classifies SQL before execution
* *THEN* the classification SHOULD distinguish unsupported-by-policy statements from unsupported-by-translation statements
* *AND* the translation design SHOULD allow future DML and DDL rules to be added as explicit supported capabilities
* *AND* the translation design SHALL NOT assume that every supported statement is a `SELECT`

<!-- DELTA:NEW -->
### Scenario: Future write translation preserves engine semantics

* *GIVEN* a future capability enables DML or DDL translation
* *WHEN* the preprocessor converts PostgreSQL-flavored write SQL to Exasol SQL
* *THEN* the conversion SHALL preserve Exasol as the executing source of truth
* *AND* the conversion SHALL reject statements whose PostgreSQL semantics cannot be mapped safely to Exasol
* *AND* the conversion SHOULD report whether failure was caused by policy, translation coverage, or Exasol execution
