# Feature: PostgreSQL-To-Exasol Compatibility Matrix

The project SHALL maintain a PostgreSQL-to-Exasol compatibility matrix that maps PostgreSQL syntax, functions, data types, commands, metadata, and session behavior to Exasol equivalents, gateway-managed behavior, or explicit unsupported status. The matrix is the source of truth for broadening the gateway from read-only queries to read/write compatibility.

## Background

* PostgreSQL 18 documents the current SQL command catalog used as the source syntax inventory.
* Exasol documentation is the source of truth for supported Exasol statements, functions, data types, SQL standard features, and documented limitations.
* Exasol supports core read, write, DDL, privilege, and transaction statements, but its syntax and behavior are not identical to PostgreSQL.
* Translation failures, unsupported PostgreSQL features, and Exasol execution failures are separate outcomes.
* The preferred translation owner is the gateway application layer, not an Exasol-side SQL preprocessor.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Every PostgreSQL command family receives a support outcome

* *GIVEN* the project evaluates PostgreSQL syntax support
* *WHEN* a PostgreSQL command family is added to the compatibility matrix
* *THEN* the matrix SHALL assign one of `direct-exasol`, `exasol-with-caveats`, `gateway-managed`, `metadata-only`, `unsupported-no-equivalent`, or `unsupported-policy`
* *AND* the matrix SHALL cite or name the corresponding Exasol statement, function, metadata object, or gateway-managed design

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Direct Exasol equivalents preserve Exasol as source of truth

* *GIVEN* a PostgreSQL statement maps to an Exasol equivalent
* *WHEN* the gateway translates the statement
* *THEN* the translated SQL SHALL execute against Exasol
* *AND* the translation SHALL preserve the meaningful operation requested by the PostgreSQL client
* *AND* the translation SHALL reject syntax whose PostgreSQL semantics cannot be mapped safely

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Core DML is mapped before broader write behavior

* *GIVEN* the project defines write support phases
* *WHEN* the first write-capable phase is specified
* *THEN* it SHOULD include explicit mappings for `INSERT`, `INSERT ... SELECT`, `UPDATE`, `DELETE`, and `MERGE` where Exasol has equivalent statements
* *AND* it SHALL include affected-row behavior, transaction behavior, error behavior, and metadata visibility for each supported statement

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: DDL support is limited to Exasol-equivalent object types

* *GIVEN* the project defines DDL support
* *WHEN* a PostgreSQL DDL command targets an object type that exists in Exasol
* *THEN* the matrix SHOULD map the command to the corresponding Exasol DDL where syntax and semantics are safe enough
* *AND* the matrix SHALL document unsupported PostgreSQL object attributes instead of silently ignoring them

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: PostgreSQL-specific engine objects remain unsupported

* *GIVEN* PostgreSQL syntax targets features such as extensions, event triggers, rewrite rules, publications, subscriptions, text search objects, foreign data wrappers, access methods, or table spaces
* *WHEN* no Exasol equivalent exists
* *THEN* the matrix SHALL mark the feature as `unsupported-no-equivalent`
* *AND* the gateway SHALL return a clear PostgreSQL-compatible error if the client sends the syntax

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Function and operator translation is explicit

* *GIVEN* a PostgreSQL expression uses a function, operator, cast, data type, or literal syntax that differs from Exasol
* *WHEN* the compatibility matrix marks it supported
* *THEN* the matrix SHALL identify the Exasol function, operator, cast, or rewrite pattern
* *AND* the translation SHALL reject type-sensitive rewrites unless the required type context is available or the rewrite is proven safe without it
* *AND* gateway-owned translation SHOULD apply the rewrite before Exasol execution so the standard path does not depend on a SQL preprocessor

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Client metadata edge cases are tracked as compatibility capabilities

* *GIVEN* a PostgreSQL-compatible client emits metadata SQL that uses PostgreSQL-specific syntax
* *WHEN* the query is required for a supported client workflow
* *THEN* the compatibility matrix SHOULD record the query family, client source, PostgreSQL construct, Exasol equivalent, and translation owner
* *AND* observed client-specific fixes SHALL be covered by fixtures before they are moved from the Exasol-side preprocessor into the gateway
* *AND* unsupported PostgreSQL metadata constructs SHALL return stable empty or `NULL` compatibility responses only when that behavior is documented

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Gateway translation preserves Exasol audit usefulness

* *GIVEN* the gateway sends already-translated SQL to Exasol
* *WHEN* an operator investigates failures through `EXA_DBA_AUDIT_SQL`
* *THEN* the Exasol audit table SHALL show the SQL sent to Exasol
* *AND* gateway diagnostics SHOULD correlate the Exasol session and statement with the original PostgreSQL SQL and translation phases
* *AND* the system SHALL document that Exasol audit no longer contains SQL preprocessor comments when preprocessor fallback is disabled

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Bulk data movement requires a separate design

* *GIVEN* PostgreSQL supports `COPY`
* *WHEN* the gateway considers exposing bulk load or export behavior
* *THEN* the project SHALL choose between PostgreSQL wire COPY behavior, Exasol `IMPORT` and `EXPORT`, client-side streaming, or explicit unsupported status
* *AND* the project SHALL document authentication, file access, connection objects, row counts, error reporting, and security implications before implementation

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Metadata compatibility reflects supported write behavior

* *GIVEN* a PostgreSQL client queries `PG_CATALOG` or `INFORMATION_SCHEMA`
* *WHEN* the gateway supports additional Exasol write or DDL capabilities
* *THEN* the metadata compatibility layer SHOULD expose the corresponding schemas, tables, views, columns, constraints, roles, privileges, routines, and command-visible objects where Exasol metadata exists
* *AND* PostgreSQL-only metadata fields SHALL remain empty or `NULL` when no Exasol equivalent exists

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Unsupported by policy is distinct from unsupported by Exasol

* *GIVEN* Exasol has a related statement or function
* *WHEN* the gateway has not yet specified safe translation, response mapping, privileges, or tests
* *THEN* the matrix SHALL mark the behavior as `unsupported-policy`
* *AND* the gateway SHALL report that the behavior is unsupported by gateway policy, not unsupported by Exasol

<!-- /DELTA:NEW -->

