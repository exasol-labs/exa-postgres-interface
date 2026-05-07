# Verification Report: Add Select Compatibility Test Framework

Status update as of 2026-04-27: after this plan landed, the suite was extended
by live client-driven fixes for DbVisualizer and DBeaver catalog browsing. The
current Exasol-side compatibility layer exposes the documented PostgreSQL 18
catalog/information-schema relation and column surface and includes targeted
preprocessor rewrites for observed metadata query shapes such as
`LATERAL UNNEST`, tuple joins, PostgreSQL array/vector subscripts,
`ARRAY_AGG`, and PostgreSQL helper functions.

## Summary of Implemented Behavior

Added a staged compatibility-harness plan plus implementation artifacts for
read-only PostgreSQL compatibility testing.

The new framework provides:

* an exhaustive JDBC `DatabaseMetaData` sweep that attempts every public method
  with deterministic sample arguments;
* persona-based SQL probe corpora for `baseline`, `dbvisualizer`, `pgjdbc`,
  `metabase`, `dbeaver`, and `analyst`;
* separation of must-pass baseline probes from exploratory compatibility probes;
* a gateway-vs-direct Exasol benchmark runner for small and medium read-only
  query pairs;
* a repository script to compile and run the suite against a supplied JDBC URL;
* a repository script to compile and run the benchmark against both the gateway
  and a direct Exasol JDBC connection;
* documentation that ties the corpus back to public pgJDBC, Metabase, and
  DBeaver sources.

## Changed Files

* `docs/smoke-test.md`
* `docs/client-compatibility-test-framework.md`
* `scripts/run_jdbc_compatibility_suite.sh`
* `scripts/run_gateway_vs_exasol_benchmark.sh`
* `tests/jdbc/PgJdbcCompatibilitySuite.java`
* `tests/jdbc/GatewayVsDirectExasolBenchmark.java`
* `specs/_plans/add-select-compatibility-test-framework/plan.md`
* `specs/_plans/add-select-compatibility-test-framework/testing/client-compatibility-harness/spec.md`

## Tests and Commands Run

* `cargo test` passed.
* Downloaded a temporary JDK 21 into `/tmp/exa-postgres-interface-jdk`.
* Compiled `tests/jdbc/PgJdbcCompatibilitySuite.java` successfully with:

```bash
/tmp/exa-postgres-interface-jdk/bin/javac \
  -cp /tmp/postgresql-42.7.8.jar \
  -d /tmp/exa-postgres-interface-jdbc-compile \
  tests/jdbc/PgJdbcCompatibilitySuite.java
```

* Compiled `tests/jdbc/GatewayVsDirectExasolBenchmark.java` successfully with:

```bash
/tmp/exa-postgres-interface-jdk/bin/javac \
  -cp /tmp/postgresql-42.7.8.jar:/tmp/exasol-jdbc-25.2.4.jar \
  -d /tmp/exa-postgres-interface-java-check \
  tests/jdbc/PgJdbcCompatibilitySuite.java \
  tests/jdbc/GatewayVsDirectExasolBenchmark.java
```

## Spec Scenarios Covered

Covered by implementation:

* The repository now has a compatibility suite that sweeps JDBC metadata
  methods and records pass/fail plus result shape.
* The SQL probe corpus now mixes must-pass and exploratory probes.
* The probe corpus reflects real client personas informed by public upstream
  sources.
* The repository now has a single-command script to compile and run the suite.
* The repository now has a benchmark runner that compares warm-query latency for
  small and medium query pairs between the gateway and direct Exasol JDBC.

## Known Gaps or Follow-Up Work

* The suite should continue to be run against a live gateway after each
  catalog/preprocessor change so newly observed client query shapes are captured
  as regression probes.
* The suite currently prints text output only. If the team wants CI ingestion,
  a follow-up could add JSON or JUnit-style output.
* The benchmark currently uses the repository sample data and warm open
  connections, so it measures query-path overhead more than heavy analytical
  throughput.
