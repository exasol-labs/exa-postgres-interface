# Feature: Exasol Sample Data for Smoke Testing

Status as of 2026-04-27: implemented and expanded. The sample data now creates
a richer demo environment with multiple users, roles, nested role grants,
schemas, tables, views, functions, scripts, constraints, and cross-schema
dependencies for metadata-browser validation.

The prototype SHALL include a repeatable way to create sample Exasol data for read-only client smoke tests and SQL translation validation. Sample data SHALL support both trivial connectivity checks and a PostgreSQL-to-Exasol conversion demo.

## Background

* Exasol Personal is the expected integration database.
* The user can provide Exasol credentials outside the repository.
* The first client smoke test uses DbVisualizer.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Sample data setup is repeatable

* *GIVEN* an accessible Exasol Personal database
* *WHEN* the operator runs the sample data setup
* *THEN* the setup SHALL create or refresh a sample table named `pg_demo.orders` or an equivalent documented table
* *AND* the setup SHALL avoid requiring secrets to be stored in the repository
* *AND* the setup SHOULD be safe to re-run during development

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Smoke-test query validates basic connectivity

* *GIVEN* sample data setup has completed
* *AND* DbVisualizer is connected through the protocol server
* *WHEN* the user runs `SELECT 1`
* *THEN* the query SHALL return a single row containing the value `1`

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Sample query validates PostgreSQL-to-Exasol conversion

* *GIVEN* sample data setup has completed
* *AND* DbVisualizer is connected through the protocol server
* *WHEN* the user runs the documented PostgreSQL-flavored sample query against `pg_demo.orders`
* *THEN* the query SHOULD exercise SQL dialect conversion through PostgreSQL-specific `::` casts and `ILIKE` case-insensitive matching
* *AND* the query SHALL return deterministic results from the sample data
* *AND* the expected result shape SHALL be documented for manual verification

<!-- /DELTA:NEW -->

