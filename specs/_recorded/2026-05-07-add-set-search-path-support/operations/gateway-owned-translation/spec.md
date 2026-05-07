# Feature: Gateway-Owned Translation Administration

The standard installation SHOULD be simple enough for an operator to deploy without installing an Exasol-side SQL preprocessor. PostgreSQL-to-Exasol dialect translation, client metadata query rewrites, and unsupported-capability errors SHOULD be owned by the gateway process, while Exasol remains the execution engine and source of durable metadata.

## Background

* The earlier implementation installed `PG_DEMO.PG_SQL_PREPROCESSOR` in Exasol and activated it for each session.
* The optional fallback preprocessor, when retained, SHOULD live in `PG_CATALOG` so PostgreSQL compatibility objects are contained in `PG_CATALOG` and `INFORMATION_SCHEMA`.
* The legacy preprocessor contains generic SQLGlot translation plus project-owned edge-case rewrites discovered through DBVisualizer, DBeaver, Qlik, Metabase, and JDBC testing.
* Installing and administering database scripts increases setup complexity, upgrade complexity, privilege requirements, and debugging surface area.
* The gateway is implemented in Rust, and Polyglot is a Rust SQL transpiler candidate with PostgreSQL and Exasol dialect support.
* Exasol-side PostgreSQL catalog compatibility views/functions may still be useful for metadata queries that should execute normally against Exasol metadata.
* Per-session state owned by the gateway (such as the active schema set via `SET search_path`) MAY require coordination with Exasol session-level verbs (`OPEN SCHEMA`) without an Exasol-side SQL preprocessor.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: search_path session state is owned by the gateway

* *GIVEN* the gateway processes PostgreSQL session commands without an Exasol-side preprocessor
* *WHEN* the gateway accepts a single-schema `SET search_path = <schema>`
* *THEN* the gateway SHALL store the active schema in per-session state alongside the Exasol session and issue `OPEN SCHEMA <schema>` against Exasol
* *AND* the gateway SHALL surface the active schema through `SHOW search_path` without sending a query to Exasol
* *AND* the gateway SHALL log the search_path classification, the resolved schema name, and the failure stage when the operation fails
* *AND* the gateway SHALL NOT require an Exasol-side SQL preprocessor or session-init script to provide this behavior
<!-- /DELTA:NEW -->
