# Feature: SQL Translator Engine Selection

The project SHALL select or revise its SQL translation engine through documented evaluation instead of assuming that the current SQLGlot preprocessor architecture is sufficient for broad read/write compatibility. The preferred direction is application-layer translation in the Rust gateway using Polyglot if it proves compatible enough. SQLGlot and hybrid translation designs MAY remain fallback or migration options, but the selected design MUST satisfy the compatibility matrix, Exasol execution requirements, and simplified administration goals.

## Background

* The current implementation uses an Exasol-side Python SQL preprocessor based on SQLGlot.
* SQLGlot is a mature Python parser and transpiler with official PostgreSQL support and community Exasol dialect support.
* Polyglot is a newer Rust-powered SQL transpiler with Rust, Python, C FFI, and TypeScript/WASM surfaces, and it lists both PostgreSQL and Exasol dialects.
* Moving translation from Exasol-side preprocessing to the application layer affects auditing, deployment, metadata rewrites, error surfaces, and debugging.
* The gateway is implemented in Rust, so native Rust translator integration is a meaningful design option.
* Reducing installation complexity is a product requirement; translation architecture SHOULD avoid required Exasol-side script objects when equivalent gateway behavior is available.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Translator candidates are evaluated against the compatibility matrix

* *GIVEN* the project considers SQLGlot, Polyglot, or another translation engine
* *WHEN* a translator candidate is evaluated
* *THEN* the evaluation SHALL run representative PostgreSQL DQL, DML, DDL, transaction, cursor-adjacent, function, type, and metadata-query fixtures from the compatibility matrix
* *AND* the evaluation SHALL compare translated SQL against Exasol parsing or execution behavior
* *AND* the evaluation SHALL record unsupported, lossy, or semantically unsafe translations
* *AND* the evaluation SHOULD include observed client metadata queries from DBVisualizer, DBeaver, Qlik, Metabase, JDBC, and `psql`

<!-- DELTA:NEW -->
### Scenario: Translator errors are safe by default

* *GIVEN* a translator encounters PostgreSQL syntax it cannot translate safely
* *WHEN* the gateway receives the translation result
* *THEN* the gateway SHALL reject the statement with a PostgreSQL-compatible error
* *AND* the gateway SHALL NOT execute best-effort translated SQL when the translator reports unsupported or lossy behavior
* *AND* diagnostics SHOULD include source SQL location information when the translator provides it

<!-- DELTA:NEW -->
### Scenario: SQLGlot remains acceptable only with proven coverage

* *GIVEN* SQLGlot is the current translation engine
* *WHEN* the project broadens from read-only queries to read/write SQL
* *THEN* SQLGlot SHALL be retained as the default only if it can translate the planned compatibility matrix safely or can be extended locally for the missing Exasol rules
* *AND* the project SHALL document any local SQLGlot dialect extensions or post-processing rewrites

<!-- DELTA:NEW -->
### Scenario: Polyglot is evaluated before replacement

* *GIVEN* Polyglot offers Rust, Python, and FFI integration surfaces
* *WHEN* the project considers replacing SQLGlot or moving translation into the application layer
* *THEN* Polyglot SHALL be evaluated for PostgreSQL-to-Exasol DQL, DML, DDL, functions, data types, errors, resource limits, packaging, and release maturity
* *AND* Polyglot SHALL NOT replace SQLGlot until the evaluation shows equal or better Exasol compatibility for planned supported statements
* *AND* Polyglot SHOULD be preferred over Exasol-side SQLGlot when it reaches fixture parity because it removes Python preprocessor installation from the standard path

<!-- DELTA:NEW -->
### Scenario: Gateway translator interface isolates library choice

* *GIVEN* the gateway may use Polyglot, SQLGlot-compatible behavior, or project-owned rewrites
* *WHEN* translation is implemented in the application layer
* *THEN* request handling SHALL call a gateway translator interface rather than a specific parser/transpiler API directly
* *AND* the interface SHALL return translated SQL, capability outcome, warnings, unsupported diagnostics, and source-location details when available
* *AND* the interface SHALL allow deterministic unit tests without connecting to Exasol

<!-- DELTA:NEW -->
### Scenario: Existing preprocessor rewrites are migration fixtures

* *GIVEN* the current Exasol-side preprocessor contains known metadata and dialect edge-case rewrites
* *WHEN* a gateway-owned translator is evaluated or implemented
* *THEN* each existing rewrite SHOULD be represented as an input/output fixture before it is ported
* *AND* fixtures SHALL include observed failures such as unsized `VARCHAR` casts, mixed `type_name, *` projections, `_pg_expandarray` metadata queries, `ANY(c.conkey)` catalog arrays, and PostgreSQL helper functions such as `pg_size_pretty`
* *AND* the gateway SHALL keep or intentionally update the expected translated SQL for each fixture

<!-- DELTA:NEW -->
### Scenario: Application-layer translation preserves existing metadata behavior

* *GIVEN* the current Exasol-side preprocessor performs metadata-query rewrites
* *WHEN* translation moves partly or fully into the Rust gateway
* *THEN* the project SHOULD move client-query rewrites into the gateway unless they MUST execute as stable Exasol metadata views/functions
* *AND* the project SHALL avoid duplicate conflicting rewrites
* *AND* the project SHALL keep translation behavior observable in logs and, where applicable, Exasol audit records

<!-- DELTA:NEW -->
### Scenario: Hybrid translation has explicit ownership boundaries

* *GIVEN* the project uses both application-layer translation and an Exasol-side preprocessor
* *WHEN* a client statement is processed
* *THEN* the spec SHALL define the owner for statement classification, PostgreSQL-to-Exasol dialect translation, metadata-query rewrites, cursor command handling, prepared statement handling, and unsupported behavior errors
* *AND* the gateway SHALL log enough information to reconstruct which layer changed or rejected the statement
* *AND* hybrid translation SHALL be treated as a migration or fallback mode unless the project documents why it is operationally preferable to gateway-owned translation

<!-- DELTA:NEW -->
### Scenario: Default translator path does not depend on Exasol SQL preprocessor

* *GIVEN* gateway-owned translation has passed the required fixture suite
* *WHEN* a new deployment starts with default configuration
* *THEN* the gateway SHALL translate supported PostgreSQL SQL before Exasol execution without requiring an Exasol SQL preprocessor
* *AND* the gateway SHALL NOT issue `ALTER SESSION SET SQL_PREPROCESSOR_SCRIPT` unless fallback mode is explicitly enabled
* *AND* documentation SHALL describe SQL preprocessor installation as optional fallback rather than mandatory setup
