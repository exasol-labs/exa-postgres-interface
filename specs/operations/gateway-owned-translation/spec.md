# Feature: Gateway-Owned Translation Administration

The standard installation SHOULD be simple enough for an operator to deploy without installing an Exasol-side SQL preprocessor. PostgreSQL-to-Exasol dialect translation, client metadata query rewrites, and unsupported-capability errors SHOULD be owned by the gateway process, while Exasol remains the execution engine and source of durable metadata.

## Background

* The earlier implementation installed `PG_DEMO.PG_SQL_PREPROCESSOR` in Exasol and activated it for each session.
* The optional fallback preprocessor, when retained, SHOULD live in `PG_CATALOG` so PostgreSQL compatibility objects are contained in `PG_CATALOG` and `INFORMATION_SCHEMA`.
* The legacy preprocessor contains generic SQLGlot translation plus project-owned edge-case rewrites discovered through DBVisualizer, DBeaver, Qlik, Metabase, and JDBC testing.
* Installing and administering database scripts increases setup complexity, upgrade complexity, privilege requirements, and debugging surface area.
* The gateway is implemented in Rust, and Polyglot is a Rust SQL transpiler candidate with PostgreSQL and Exasol dialect support.
* Exasol-side PostgreSQL catalog compatibility views/functions may still be useful for metadata queries that should execute normally against Exasol metadata.

## Scenarios

### Scenario: Standard install avoids required SQL preprocessing

* *GIVEN* an operator installs the gateway for a supported Exasol database
* *WHEN* the operator follows the standard installation path
* *THEN* the installation SHOULD NOT require creating any Exasol SQL preprocessor
* *AND* the gateway SHALL NOT require `ALTER SESSION SET SQL_PREPROCESSOR_SCRIPT` for normal SQL translation
* *AND* the installation SHALL document any remaining required Exasol-side compatibility schemas, views, functions, or privileges


### Scenario: Gateway owns SQL translation before Exasol execution

* *GIVEN* a PostgreSQL client sends SQL through the gateway
* *WHEN* the statement is classified as Exasol-executable
* *THEN* the gateway SHALL translate PostgreSQL dialect syntax to Exasol dialect syntax before sending SQL to Exasol
* *AND* Exasol SHALL receive SQL that is intended to parse without relying on a SQL preprocessor
* *AND* the gateway SHALL reject unsupported or unsafe translations before any SQL is sent to Exasol


### Scenario: Existing preprocessor edge cases become portable gateway rules

* *GIVEN* the current Exasol-side preprocessor contains project-owned edge-case rewrites
* *WHEN* translation ownership moves into the gateway
* *THEN* every known rewrite used for DBVisualizer, DBeaver, Qlik, Metabase, JDBC, PostgreSQL catalog, and `INFORMATION_SCHEMA` compatibility SHOULD be represented as a gateway translation fixture
* *AND* the gateway SHALL preserve the observable compatibility behavior or document an intentional behavior change
* *AND* the migration SHALL NOT discard a preprocessor rewrite unless tests show it is obsolete


### Scenario: Polyglot is wrapped behind a gateway translator interface

* *GIVEN* the gateway evaluates Polyglot for PostgreSQL-to-Exasol translation
* *WHEN* Polyglot is used by the gateway
* *THEN* the gateway SHALL call Polyglot through an internal translator interface rather than coupling request handling directly to the library API
* *AND* the translator interface SHALL expose translated SQL, warnings, unsupported diagnostics, source-location details when available, and resource-limit failures
* *AND* the gateway SHALL allow project-owned pre- and post-translation rewrite steps around Polyglot


### Scenario: Translation behavior is observable without Exasol preprocessor audit rows

* *GIVEN* translation occurs in the gateway instead of an Exasol-side preprocessor
* *WHEN* a statement succeeds or fails
* *THEN* gateway logs SHOULD include a correlation identifier, statement class, capability outcome, translator engine, rewrite phases applied, and failure stage
* *AND* logs SHALL avoid plaintext secrets and SHOULD avoid logging full SQL by default when SQL MAY contain sensitive literals
* *AND* a debug mode MAY expose full original and translated SQL for controlled troubleshooting


### Scenario: Database-side compatibility objects are separated from translation

* *GIVEN* a client metadata query requires PostgreSQL-shaped catalog objects
* *WHEN* the query can execute safely against Exasol-backed compatibility views or functions
* *THEN* those views or functions MAY remain Exasol-side database objects
* *AND* their installation SHALL be idempotent and versioned independently from the gateway binary
* *AND* translation of the client SQL that references those objects SHOULD still occur in the gateway


### Scenario: Interactive first-run bootstrap installs compatibility objects

* *GIVEN* an operator starts the gateway binary from a terminal
* *AND* the selected configuration file does not exist
* *WHEN* the gateway starts
* *THEN* it SHOULD prompt for the configuration values required to connect the gateway to Exasol and listen for PostgreSQL clients
* *AND* it SHOULD write a TOML configuration file without saving database credentials
* *AND* it SHOULD ask for temporary Exasol setup credentials only after reminding the operator that those credentials are used only for catalog installation
* *AND* it SHOULD check whether `PG_CATALOG` and `INFORMATION_SCHEMA` compatibility objects are present
* *AND* it SHALL ask permission before creating or refreshing compatibility objects
* *AND* it SHOULD print systemd service guidance for running subsequent non-interactive starts with the saved configuration


### Scenario: Exasol-side preprocessor remains an optional fallback during migration

* *GIVEN* a deployment needs behavior that gateway-owned translation does not yet cover
* *WHEN* the operator explicitly enables preprocessor fallback
* *THEN* the gateway MAY initialize `PG_CATALOG.PG_SQL_PREPROCESSOR` for that deployment
* *AND* fallback mode SHALL be visibly reported in startup logs and session diagnostics
* *AND* fallback mode SHALL NOT be the default once gateway-owned translation reaches fixture parity


### Scenario: Upgrades avoid manual database-script drift

* *GIVEN* a new gateway version changes translation behavior
* *WHEN* the operator upgrades the gateway binary
* *THEN* translation changes SHOULD ship with the binary and not require manual replacement of a SQL preprocessor script
* *AND* any required database compatibility object upgrade SHALL be reported by a version check or installer command
* *AND* the gateway SHOULD fail clearly when required compatibility object versions are missing or stale
