# Feature: Client Compatibility Harness

Status as of 2026-04-27: implemented. The harness includes JDBC metadata
sweeps, persona query corpora, and a gateway-vs-direct Exasol benchmark. It is
the preferred regression path after metadata compatibility or SQL preprocessor
changes.

The repository SHALL provide a repeatable compatibility harness for the PostgreSQL gateway. The harness SHALL report which JDBC metadata calls and PostgreSQL-flavored statement families succeed, fail, or degrade for realistic client personas without assuming full PostgreSQL compatibility. The repository SHALL also provide a repeatable latency benchmark that compares gateway query execution against direct Exasol JDBC for logically equivalent read-heavy queries.

## Background

* The gateway began as a read-mostly PostgreSQL compatibility layer in front of Exasol.
* The first implementation already has narrow smoke coverage for pgJDBC and DbVisualizer.
* Real PostgreSQL clients rely on both JDBC metadata APIs and direct `pg_catalog` or `information_schema` queries.
* The team wants to know which PostgreSQL statement families do not work yet, not only which smoke queries already pass.
* The team also wants to understand gateway overhead for representative read-heavy queries compared with direct Exasol JDBC.

## Scenarios

### Scenario: Compatibility suite sweeps JDBC metadata exhaustively

* *GIVEN* a running gateway reachable through PostgreSQL JDBC
* *WHEN* the operator runs the compatibility suite
* *THEN* the suite SHALL attempt every public `java.sql.DatabaseMetaData` method with deterministic sample arguments
* *AND* the suite SHALL record whether each method passed or failed
* *AND* the suite SHALL record result-set shape or scalar return values when the call succeeds
* *AND* the suite SHALL record SQLState and failure details when the call fails


### Scenario: Persona query corpora mix must-pass and exploratory probes

* *GIVEN* the compatibility suite has a configured sample catalog, schema, and table
* *WHEN* the suite runs its SQL probes
* *THEN* the suite SHALL include a small must-pass baseline for current gateway smoke behavior
* *AND* the suite SHALL include exploratory probes for additional PostgreSQL client personas and stress cases
* *AND* exploratory failures SHALL be reported without being treated as must-pass regressions by default


### Scenario: Query personas reflect real PostgreSQL clients

* *GIVEN* the team wants representative client coverage
* *WHEN* the suite defines SQL probe families
* *THEN* the suite SHALL include query families informed by pgJDBC, DbVisualizer, Metabase, and DBeaver behavior
* *AND* the documentation SHALL identify which upstream tools informed each persona
* *AND* the suite MAY add analyst-oriented PostgreSQL `SELECT` stress cases beyond the observed client corpora
* *AND* the suite SHOULD also include representative DML, DDL, transaction, session, and utility operations so unsupported non-read behavior is visible in the same report


### Scenario: SQL probes cover mixed statement kinds

* *GIVEN* PostgreSQL clients issue more than just `SELECT` statements
* *WHEN* the compatibility suite runs its SQL corpus
* *THEN* the suite SHALL support probes that return result sets and probes that return update counts or no rows
* *AND* the suite SHALL support DQL, DML, DDL, transaction-control, session, and utility statements in the same framework
* *AND* the suite SHOULD allow per-probe setup and cleanup SQL when a statement family requires scratch objects or transaction context


### Scenario: Individual probe failures do not stop discovery

* *GIVEN* one metadata call or SQL probe fails
* *WHEN* later probes remain runnable
* *THEN* the suite SHALL continue collecting outcomes for the remaining probes
* *AND* the final report SHALL separate must-pass failures from exploratory failures
* *AND* the report SHOULD help the team identify unsupported PostgreSQL statement families instead of hiding them behind the first exception


### Scenario: Operators can run the suite with a single command

* *GIVEN* the operator has a PostgreSQL JDBC URL and Exasol credentials
* *WHEN* they run the documented compatibility-suite command
* *THEN* the repository SHALL provide a script or equivalent documented entry point that compiles and runs the suite
* *AND* the run path SHALL support the extended JDBC query mode used by many Java tools


### Scenario: Benchmark compares gateway latency with direct Exasol JDBC

* *GIVEN* the operator has both a PostgreSQL gateway JDBC URL and a direct Exasol JDBC URL that reach the same sample data
* *WHEN* they run the documented benchmark command
* *THEN* the repository SHALL execute logically equivalent read-heavy query pairs against both targets
* *AND* the benchmark SHALL measure execution latency over repeated warm-connection runs
* *AND* the report SHALL include summary statistics for both targets plus a gateway-over-direct overhead ratio


### Scenario: Benchmark validates result equivalence before comparing latency

* *GIVEN* the benchmark runs equivalent query pairs against the gateway and direct Exasol
* *WHEN* the pair completes successfully on both sides
* *THEN* the benchmark SHALL compare a deterministic result digest for both targets
* *AND* the benchmark SHALL flag pairs where the gateway and direct Exasol results do not match
* *AND* the latency comparison SHOULD remain tied to logically equivalent successful query results
