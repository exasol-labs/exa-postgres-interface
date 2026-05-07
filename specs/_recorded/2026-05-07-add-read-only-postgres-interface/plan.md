# Plan: Add Read-Only PostgreSQL Interface

Status as of 2026-04-27: implemented. The active gateway is a Rust/`pgwire`
binary, SQL translation runs inside Exasol through
`PG_DEMO.PG_SQL_PREPROCESSOR`, and PostgreSQL metadata compatibility is now
provided by Exasol-side `PG_CATALOG` and `INFORMATION_SCHEMA` schemas. The
systemd deployment path is documented in `README.md`.

Future target scope note: the read-only policy in this implemented prototype
plan is superseded by `specs/_plans/change-postgres-read-write-compatibility/`,
which defines the next design direction as capability-based read/write
PostgreSQL compatibility where Exasol has an equivalent or the gateway can
provide explicit compatibility behavior.

## Objective

Build the first useful prototype path for connecting PostgreSQL-compatible client tools to Exasol through this server.

The first customer-visible success case is that DbVisualizer can use its PostgreSQL connector to connect through the interface to Exasol, authenticate with user-provided credentials, run `SELECT 1`, run read-only DQL queries against sample data, and receive tabular results. The plan should keep Tableau in mind as a future PostgreSQL-compatible client, but DbVisualizer is the first validation target.

The first implementation is intentionally read-only, but the architecture SHOULD be built as a general PostgreSQL-to-Exasol session and statement gateway. It SHOULD keep statement classification, transaction handling, protocol response mapping, SQL translation, and authorization boundaries extensible enough to add write support later without replacing the core server design.

Research into PostgreSQL-compatible database interfaces shows that read/write support is common, but the compatibility level varies by engine. Systems such as CockroachDB and YugabyteDB expose broad DML and transaction support, while analytical or streaming systems such as ClickHouse, CrateDB, QuestDB, RisingWave, Materialize, and Redshift support write behavior with engine-specific limits. The Exasol interface SHOULD follow the same pattern: expose PostgreSQL wire-protocol compatibility as an explicit compatibility layer over Exasol behavior, not as a claim of full PostgreSQL semantics.

## Relevant Existing Specs

* `specs/mission.md`

There are no permanent feature specs yet. This plan introduces staged delta specs for the initial protocol, SQL translation, runtime, and sample-data behavior.

## User-Side Scope Decisions

* Initial query scope is read-only DQL.
* Read/write support is a future goal; the first implementation SHOULD avoid read-only-specific architecture that would block later DML or DDL support.
* PostgreSQL wire compatibility SHOULD be treated as a protocol and tooling integration layer; SQL semantics remain Exasol semantics unless explicitly translated or documented.
* `SELECT 1` MUST work as the simplest smoke test.
* The prototype SHOULD include a sample query that exercises PostgreSQL-to-Exasol SQL conversion.
* DbVisualizer is the first client target.
* Tableau is a compatibility consideration, but not a required first smoke-test client.
* Credentials entered by the client SHOULD be used to connect to Exasol, unless implementation research shows a safer or more feasible prototype constraint.
* Unsupported behavior SHOULD produce a warning plus a clear PostgreSQL-style error.
* The preferred operational shape is an installable binary configured as a systemd service.
* Exasol Personal is the expected integration database; sample data needs to be created for tests.

## Proposed Spec Deltas

* `protocol/read-only-query-path/spec.md`
  * Defines the observable PostgreSQL-compatible startup, authentication, query execution, result, unsupported-behavior path, and future write-support extension boundaries.
* `sql/postgres-to-exasol-translation/spec.md`
  * Defines read-only SQL translation through the Exasol-side Python preprocessor and sample conversion coverage.
* `operations/service-runtime/spec.md`
  * Defines binary, configuration, logging, and systemd service expectations for the prototype.
* `testing/exasol-sample-data/spec.md`
  * Defines sample schema/data requirements for repeatable smoke and integration testing.

## PostgreSQL-Compatible Interface Lessons

Comparable systems show several implementation patterns that SHOULD inform this prototype:

* The PostgreSQL wire protocol can carry both read and write statements; read-only behavior is a product-scope choice, not a protocol limitation.
* Mature PostgreSQL-compatible systems separate protocol compatibility from SQL semantic compatibility and document differences.
* Write support usually requires more than forwarding SQL: command completion tags, affected-row counts, transaction/autocommit behavior, error recovery, metadata visibility, and prepared statement behavior all become observable to clients.
* Some systems accept transaction commands for client compatibility even when their engine semantics differ; this prototype SHOULD avoid silent transaction emulation unless the behavior is explicitly documented and verified against Exasol.
* Analytical engines often support `INSERT` earlier than `UPDATE` or `DELETE`; future Exasol write support MAY be introduced by capability class instead of as all-or-nothing PostgreSQL compatibility.

## Canonical Smoke Queries

Basic connectivity query:

```sql
SELECT 1;
```

PostgreSQL-to-Exasol conversion demo query:

```sql
SELECT
  order_id,
  order_ts::DATE AS order_date,
  amount::DECIMAL(18, 2) AS amount_eur
FROM pg_demo.orders
WHERE customer_name ILIKE 'acme%'
ORDER BY order_id
LIMIT 3;
```

Expected result shape:

```text
order_id | order_date | amount_eur
```

The demo query intentionally uses PostgreSQL-specific `::` casts and `ILIKE` case-insensitive matching. The implementation should verify the exact converted Exasol SQL through the selected `sqlglot` version and the running Exasol database before treating this as the stable acceptance query.

## Implementation Tasks

1. Choose the server implementation stack.
   * Evaluate PostgreSQL wire-protocol server support, Exasol client support, Linux binary packaging, and systemd fit.
   * Prefer an implementation strategy that can later support DML, DDL, transactions, prepared statements, and richer metadata behavior without replacing the listener/session architecture.
   * Prefer libraries or internal boundaries that expose PostgreSQL command tags, affected-row counts, transaction status, and extended-query protocol states explicitly.
   * Candidate directions include a Go or Rust long-running server process, or a Python prototype if protocol support and packaging are acceptable.
   * Document the selection in the implementation notes before committing to code.

2. Establish project scaffolding.
   * Add source tree, dependency metadata, formatting, linting, and test commands for the selected runtime.
   * Add local development configuration examples without committing secrets.

3. Implement configuration loading.
   * Include listen address/port.
   * Include Exasol endpoint configuration.
   * Include logging level/output.
   * Include Python preprocessor installation or selection settings.
   * Support passing client-supplied username/password to Exasol.

4. Implement minimum PostgreSQL wire-protocol server behavior.
   * Accept startup messages from DbVisualizer.
   * Support username/password authentication.
   * Establish one Exasol session per PostgreSQL client session.
   * Handle simple query execution for read-only statements.
   * Return row descriptions, data rows, command completion, ready state, and PostgreSQL-compatible errors.
   * Keep the internal statement execution interface independent of the first read-only guardrail so future command classes can be added deliberately.
   * Model execution results as either row-returning results, command-completion results, or errors so later write statements do not need a separate execution path.

5. Implement Exasol session initialization.
   * Install or activate the Python `sqlglot` preprocessor for each session.
   * Make initialization observable in logs.
   * Fail the client connection with a clear error if initialization cannot complete.

6. Implement read-only SQL guardrails.
   * Permit DQL required for smoke tests.
   * Reject DDL, DML, and other unsupported commands with warning logs and PostgreSQL-style errors.
   * Avoid silently emulating PostgreSQL behavior when Exasol behavior differs materially.
   * Implement rejection as an explicit policy decision rather than hard-coding the entire execution path as SELECT-only.
   * Record unsupported-by-policy rejections separately from unsupported-by-translation and unsupported-by-Exasol failures.

7. Create sample data setup.
   * Provide a repeatable script or documented command for creating sample Exasol objects.
   * Create `pg_demo.orders` or an equivalent documented sample table.
   * Include the canonical PostgreSQL-to-Exasol conversion query from this plan.
   * Keep credentials out of the repository.

8. Add packaging/service prototype.
   * Build an installable binary or equivalent runtime artifact.
   * Provide a systemd unit template and configuration example.
   * Document startup, shutdown, and log inspection.

9. Verify with integration and manual client smoke tests.
   * Run automated tests where practical.
   * Validate against Exasol Personal.
   * Manually connect with DbVisualizer's PostgreSQL connector.
   * Run `SELECT 1` and the sample conversion query.

## Verification Plan

Automated verification SHOULD include:

* Unit tests for configuration loading and validation.
* Unit tests for read-only SQL classification where implemented outside Exasol.
* Unit tests that classify DQL, DML, DDL, transaction commands, and client metadata queries even when only DQL is enabled.
* Unit tests for error mapping.
* Unit tests for row-returning versus command-completion result handling.
* Integration tests that connect through the protocol server to Exasol Personal.
* SQL translation examples covering the sample conversion query.

Manual verification MUST include:

* Start the server locally with non-secret configuration.
* Connect from DbVisualizer using the PostgreSQL connector.
* Authenticate with credentials that are accepted by Exasol.
* Run `SELECT 1`.
* Run the sample conversion query against the sample data.
* Confirm unsupported write or DDL statements return a visible client error and produce a server warning.
* Confirm rejected write statements leave the client connection usable for a later supported `SELECT`.

Tableau verification MAY be added after the DbVisualizer path works.

## Risks and Assumptions

* DbVisualizer may issue PostgreSQL metadata or system catalog queries during connection and browsing; the first implementation may need to support or explicitly reject these queries.
* Tableau may require broader PostgreSQL protocol and metadata behavior than DbVisualizer.
* Future write support will require transaction semantics, autocommit behavior, update counts, error recovery, and SQL translation rules that are not part of the first read-only acceptance path.
* PostgreSQL-compatible competitors vary widely in transaction fidelity; this prototype should document exact Exasol behavior instead of imitating PostgreSQL behavior loosely.
* Future support may add `INSERT` before broader write support if that best matches Exasol and client-tool needs.
* Passing client credentials through to Exasol is user-friendly, but implementation details depend on the selected Exasol driver and authentication support.
* Exasol Python preprocessor setup may require database privileges or environment preparation that must be documented.
* `sqlglot` coverage may not fully translate all PostgreSQL DQL constructs to Exasol; unsupported conversions need clear diagnostics.
* Binary packaging and systemd service setup may be staged after the first developer-run prototype if protocol and Exasol integration risks dominate.

## Open Decisions

* Which implementation language and PostgreSQL wire-protocol library should be used?
* Which Exasol driver or protocol should the server use?
* Which exact PostgreSQL metadata queries does DbVisualizer send during first connect and browse?
* Which PostgreSQL transaction and autocommit behaviors should be mapped first when write support is added later?
* Should future write support start with `INSERT` only, or should `INSERT`, `UPDATE`, and `DELETE` be planned together?
* Does the selected `sqlglot` version translate the canonical demo query into SQL accepted by the target Exasol Personal version?
* How will the Python preprocessor script be installed, updated, and selected per session?
* What minimum Exasol Personal setup and credentials will be available for integration testing?
