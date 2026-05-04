# Client Compatibility Test Framework

This repository now includes a JDBC-driven compatibility suite for the
PostgreSQL gateway. The goal is not only to keep a smoke test green, but to
show which JDBC metadata calls and PostgreSQL-flavored statement families work,
which fail, and which fail only for specific client personas.

Current status:

* The suite is intended to be run against a deployed gateway with the
  Exasol-side `PG_CATALOG`, `INFORMATION_SCHEMA`, and
  `PG_DEMO.PG_SQL_PREPROCESSOR` objects installed.
* The current compatibility layer has been iterated against real DbVisualizer
  and DBeaver metadata-browser failures observed in `SYS.EXA_DBA_AUDIT_SQL`.
* Metadata coverage now includes the documented PostgreSQL 18 catalog and
  information-schema relation/column surface, with unsupported PostgreSQL-only
  features represented by empty or `NULL`-filled compatibility views.

## What It Covers

The framework has three layers:

* An exhaustive `DatabaseMetaData` sweep that attempts every public
  `java.sql.DatabaseMetaData` method with deterministic sample arguments and
  records pass/fail, SQLState, and result shape.
* A persona query corpus with must-pass baseline probes plus exploratory probes
  drawn from real PostgreSQL clients, analytical SQL patterns, and non-read
  statement families.
* A gateway-vs-direct Exasol latency benchmark for small probes, result-set
  transfer ladders, and heavier analytical queries that return either many rows
  or only a single row.

Current personas:

* `baseline`: the gateway's existing must-pass smoke path.
* `dbvisualizer`: catalog and browse queries already documented in this repo.
* `pgjdbc`: helper queries used by PostgreSQL JDBC metadata code.
* `metabase`: metadata-sync and query-result-metadata patterns, especially
  `LIMIT 0` and `LIMIT 1` probes.
* `dbeaver`: direct `pg_catalog` schema browser queries.
* `analyst`: PostgreSQL-flavored read-only stress queries that may expose
  translation or semantic gaps.
* `dml`: `INSERT`, `UPDATE`, `DELETE`, and `MERGE` probes.
* `ddl`: `CREATE`, `ALTER`, `TRUNCATE`, and `DROP` probes using scratch
  objects when needed.
* `transaction`: `BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`, and related
  transaction-control probes.
* `session`: `SET`, `SHOW`, `RESET`, and similar session-state probes.
* `utility`: `EXPLAIN`, `COPY`, `LOCK`, `VACUUM`, `ANALYZE`, and related
  PostgreSQL utility-command probes.

Recent client-driven compatibility fixes covered:

* PostgreSQL helper functions such as `format_type`, `pg_get_constraintdef`,
  `pg_get_expr`, `pg_get_viewdef`, `to_regclass`, and `oidvectortypes`.
* DbVisualizer metadata queries using `ARRAY_AGG`, tuple joins,
  `LATERAL UNNEST`, PostgreSQL vector subscripts, and `pg_foreign_server`
  aliases that conflict with Exasol parsing.
* DBeaver PostgreSQL plugin catalog probes for system views and helper
  functions outside the documented base catalog tables.

## Upstream Research Inputs

The current corpus is based on these public sources:

* pgJDBC `PgDatabaseMetaData` source:
  [PgDatabaseMetaData.java](https://github.com/pgjdbc/pgjdbc/blob/master/pgjdbc/src/main/java/org/postgresql/jdbc/PgDatabaseMetaData.java?plain=1)
* Metabase driver docs:
  [metabase.driver](https://metabase-dev-docs.github.io/metabase/metabase.driver.html)
* DBeaver PostgreSQL source:
  [PostgreDatabase.java](https://github.com/dbeaver/dbeaver/blob/devel/plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/model/PostgreDatabase.java),
  [PostgreSchema.java](https://github.com/dbeaver/dbeaver/blob/devel/plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/model/PostgreSchema.java)

Relevant behaviors observed from those sources:

* pgJDBC metadata touches `pg_settings`, `pg_type`, `pg_namespace`,
  `pg_class`, `pg_attribute`, `pg_proc`, `pg_index`, and related catalog
  helpers.
* Metabase relies heavily on JDBC metadata such as `.getTables` and
  `.getColumns`, and its query-result-metadata path commonly uses `LIMIT 0` or
  `LIMIT 1` wrappers.
* DBeaver performs direct schema-browser queries against `pg_namespace`,
  `pg_class`, `pg_attribute`, `pg_constraint`, and `pg_index`, often with
  parameterized schema or OID lookups.

## Run It

Install the Exasol-side compatibility SQL, start the gateway, then run:

```bash
scripts/run_jdbc_compatibility_suite.sh \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys \
  'EXASOL_PASSWORD'
```

Useful options:

```bash
--catalog=exasol
--schema=PG_DEMO
--table=ORDERS
--column-pattern=%
--personas=baseline,dbvisualizer,metabase,dbeaver
--strict
--output=/tmp/exa-postgres-compat.txt
```

`--strict` only fails the process when must-pass baseline probes fail.
Exploratory failures are still reported, but they stay informational by
default. This matters more now that the corpus includes statement families that
the gateway may still reject by design or by current implementation limits.

To compare query latency against a direct Exasol JDBC connection:

```bash
scripts/run_gateway_vs_exasol_benchmark.sh \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys \
  'EXASOL_PASSWORD' \
  'jdbc:exa:127.0.0.1:8563' \
  sys \
  'EXASOL_PASSWORD'
```

Useful benchmark options:

```bash
--warmup=3
--iterations=10
--output=/tmp/exa-postgres-benchmark.txt
--families=transfer-few-cols,transfer-many-cols,analytic,analytic-one-row
--labels=transfer-many-cols-1000000
--skip-validation
```

The benchmark measures query latency on already-open JDBC connections. It does
not measure connection-establishment time.

`--skip-validation` is useful when you want a true first-execution timing on a
fresh session and do not want the runner to execute the same SQL once before
measurement.

## Reading The Report

The suite prints three result kinds:

* `PASS`: the metadata call or SQL probe succeeded.
* `FAIL`: the call or probe failed. For SQL failures, the suite prints SQLState
  and the driver-visible error message.
* `SKIP`: argument mapping was not available for that metadata method.

The summary separates must-pass failures from exploratory failures so the team
can keep core smoke coverage stable while still learning about unsupported
PostgreSQL surface area.

SQL probes are no longer limited to result sets. A probe may report:

* `statement=resultset` with result shape and sample rows.
* `statement=update` with JDBC update count and whether additional results were
  exposed.

The benchmark report prints, for each query pair:

* direct Exasol latency statistics;
* gateway latency statistics;
* gateway-over-direct ratio;
* a deterministic result digest comparison to guard against comparing unlike
  results.

## Performance Profile To Expect

On the current benchmark host, the gateway showed three distinct regimes:

* Tiny result sets: a roughly fixed `140-155 ms` overhead per query.
* Large transfers: an additional payload-dependent penalty, with observed
  gateway-over-direct ratios from about `1.11x` to `1.38x` on `1M-10M` row
  transfers depending on row width.
* Heavy compute with tiny results: near-direct timings. One-row analytical
  workloads taking about `2.5s`, `5.6s`, and `20s` in Exasol stayed within a
  few hundred milliseconds of direct JDBC.

That means the gateway is not well described by a single rule such as
"always adds 1-2 seconds" or "always costs 25%". The right mental model is:

* small-query fixed tax;
* transfer cost that scales with returned data volume;
* little additional penalty when Exasol runtime dominates and only a few rows
  come back.

## Likely Gap Areas

The suite is designed to confirm these, not assume them, but the current code
and upstream client behavior suggest that these PostgreSQL features are
especially important to watch:

* Rich metadata methods beyond the paths already covered by DbVisualizer,
  DBeaver, pgJDBC, and the existing smoke tests.
* DML/DDL paths that may still be rejected by gateway policy, translation
  limits, or Exasol semantic differences.
* Queries that depend on PostgreSQL catalog helper functions such as
  `pg_get_expr`, `pg_get_constraintdef`, or other `pg_catalog` helpers in
  shapes not yet observed from real clients.
* PostgreSQL-specific `SELECT` features such as `DISTINCT ON`, `FILTER`,
  arrays, JSON builders, and other constructs that may not translate cleanly to
  Exasol through the current SQL preprocessor path.
* Gateway latency overhead that is acceptable for small smoke queries but may be
  more visible on medium queries where protocol translation and metadata mapping
  dominate.

Those points are an inference from the current implementation plus the public
client sources above; the suite exists to replace that inference with concrete
results.
