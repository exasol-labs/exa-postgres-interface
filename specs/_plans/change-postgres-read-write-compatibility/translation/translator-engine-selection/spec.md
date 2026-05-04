# Feature: SQL Translator Engine Selection

The project SHALL select or revise its SQL translation engine through documented evaluation instead of assuming that the current SQLGlot preprocessor architecture is sufficient for broad read/write compatibility. SQLGlot, Polyglot, and hybrid translation designs MAY be considered, but the selected design MUST satisfy the compatibility matrix and Exasol execution requirements.

## Background

* The current implementation uses an Exasol-side Python SQL preprocessor based on SQLGlot.
* SQLGlot is a mature Python parser and transpiler with official PostgreSQL support and community Exasol dialect support.
* Polyglot is a newer Rust-powered SQL transpiler with Rust, Python, C FFI, and TypeScript/WASM surfaces, and it lists both PostgreSQL and Exasol dialects.
* Moving translation from Exasol-side preprocessing to the application layer affects auditing, deployment, metadata rewrites, error surfaces, and debugging.
* The gateway is implemented in Rust, so native Rust translator integration is a meaningful design option.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Translator candidates are evaluated against the compatibility matrix

* *GIVEN* the project considers SQLGlot, Polyglot, or another translation engine
* *WHEN* a translator candidate is evaluated
* *THEN* the evaluation SHALL run representative PostgreSQL DQL, DML, DDL, transaction, cursor-adjacent, function, type, and metadata-query fixtures from the compatibility matrix
* *AND* the evaluation SHALL compare translated SQL against Exasol parsing or execution behavior
* *AND* the evaluation SHALL record unsupported, lossy, or semantically unsafe translations

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

<!-- DELTA:NEW -->
### Scenario: Application-layer translation preserves existing metadata behavior

* *GIVEN* the current Exasol-side preprocessor performs metadata-query rewrites
* *WHEN* translation moves partly or fully into the Rust gateway
* *THEN* the project SHALL define which rewrites remain Exasol-side and which move application-side
* *AND* the project SHALL avoid duplicate conflicting rewrites
* *AND* the project SHALL keep translation behavior observable in logs and, where applicable, Exasol audit records

<!-- DELTA:NEW -->
### Scenario: Hybrid translation has explicit ownership boundaries

* *GIVEN* the project uses both application-layer translation and an Exasol-side preprocessor
* *WHEN* a client statement is processed
* *THEN* the spec SHALL define the owner for statement classification, PostgreSQL-to-Exasol dialect translation, metadata-query rewrites, cursor command handling, prepared statement handling, and unsupported behavior errors
* *AND* the gateway SHALL log enough information to reconstruct which layer changed or rejected the statement
