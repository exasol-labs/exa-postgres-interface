# Plan: Add Select Compatibility Test Framework

Status as of 2026-04-27: implemented. The repository includes JDBC smoke tests,
a broader JDBC compatibility suite, and a gateway-vs-direct Exasol benchmark.
The suite should be run against a deployed gateway whenever catalog or
preprocessor compatibility changes.

## Objective

Build a reusable compatibility test framework for the PostgreSQL gateway so the team can answer these questions with one run:

* Which JDBC `DatabaseMetaData` methods work, fail, or degrade through the gateway?
* Which PostgreSQL-flavored statement families work for realistic client personas such as DbVisualizer, pgJDBC, Metabase, and DBeaver?
* Which unsupported DQL, DML, DDL, transaction, session, and utility operations should be treated as implementation gaps instead of one-off smoke-test misses?
* How much latency overhead does the PostgreSQL gateway add for representative read-heavy workloads compared with a direct Exasol JDBC connection?

The framework SHOULD favor discovery and reporting over brittle pass/fail assertions. It MUST keep a small must-pass baseline for regression detection, but it SHOULD also run exploratory probes that surface unsupported PostgreSQL queries without freezing the current limitations into unit tests.

## Relevant Existing Specs

* `specs/mission.md`
* `specs/_plans/add-read-only-postgres-interface/protocol/read-only-query-path/spec.md`
* `specs/_plans/add-read-only-postgres-interface/testing/exasol-sample-data/spec.md`

There are no permanent testing specs yet. This plan introduces a staged testing spec for compatibility-harness behavior.

## User-Side Scope Decisions

* Scope now includes JDBC metadata plus PostgreSQL DQL, DML, DDL, transaction control, session, and utility statement families.
* The framework SHOULD test current behavior without assuming full PostgreSQL semantics or full write support.
* The framework SHOULD separate must-pass baseline probes from exploratory compatibility probes.
* The framework SHOULD reflect real client behavior drawn from open-source PostgreSQL tooling.
* The framework SHOULD make unsupported statement families visible in one report instead of stopping at the first failure.
* The framework SHOULD include a latency benchmark that compares gateway execution against direct Exasol JDBC for logically equivalent read-heavy queries.

## Research Inputs

The persona corpus should be informed by real client implementations and docs:

* pgJDBC `PgDatabaseMetaData` issues catalog queries against relations such as `pg_namespace`, `pg_class`, `pg_attribute`, `pg_settings`, `pg_type`, `pg_proc`, and related metadata helpers.
* Metabase driver docs describe metadata sync around `.getTables`, `.getColumns`, FK/index discovery, and query-result metadata paths that commonly use `LIMIT 0` or `LIMIT 1`.
* DBeaver PostgreSQL source uses direct `pg_catalog` queries for schemas, relations, columns, constraints, and indexes, often with parameterized OID-based lookups.
* DbVisualizer profile queries already documented in this repository remain part of the baseline browse path.

## Proposed Spec Deltas

* `testing/client-compatibility-harness/spec.md`
  * Defines the observable behavior of the compatibility harness, metadata sweep, persona query corpus, latency benchmark, and reporting model.

## Implementation Tasks

1. Add a staged testing spec for the compatibility harness.
2. Add a JDBC compatibility suite that:
   * sweeps all public `DatabaseMetaData` methods with deterministic sample arguments;
   * captures pass/fail outcome, SQLState, result-set shape, and sample rows or scalar values;
   * keeps wrapper/plumbing calls visible so the sweep is exhaustive.
3. Add persona query corpora for:
   * baseline gateway smoke behavior;
   * DbVisualizer browse queries already observed in this repo;
   * pgJDBC metadata-helper queries;
   * Metabase-style metadata and `LIMIT 0` / `LIMIT 1` query-metadata probes;
   * DBeaver-style schema browser queries;
   * analyst-oriented PostgreSQL-flavored `SELECT` stress cases;
   * DML probes such as `INSERT`, `UPDATE`, `DELETE`, and `MERGE`;
   * DDL probes such as `CREATE`, `ALTER`, `TRUNCATE`, and `DROP`;
   * transaction/session/utility probes such as `BEGIN`, `SAVEPOINT`, `SET`, `SHOW`, `EXPLAIN`, `COPY`, `VACUUM`, and `ANALYZE`.
4. Add a run script that compiles and executes the compatibility suite against a supplied JDBC URL and credentials.
5. Add a gateway-vs-direct Exasol benchmark that:
   * runs a small set of logically equivalent small and medium read-only query pairs;
   * measures warm and repeated execution latency on existing open connections;
   * reports row/result consistency plus summary statistics such as average, median, and p95 latency;
   * reports gateway overhead relative to direct Exasol JDBC.
6. Document the framework, persona sources, run commands, benchmark assumptions, and likely unsupported query families that deserve implementation attention.

## Verification Plan

Automated verification SHOULD include:

* `cargo test` to ensure repository-local verification still passes.
* Static review of the new query corpus against the current sample data and metadata layer.
* Java compilation of the new suite when a local JDK is available.
* Java compilation of the new benchmark runner when a local JDK is available.

Manual verification SHOULD include:

* running the compatibility suite against a live gateway with `preferQueryMode=extended`;
* reviewing the must-pass baseline failures separately from exploratory failures;
* reviewing which metadata methods and statement families fail with clear SQLState/message details.
* running the latency benchmark against both the gateway and a direct Exasol JDBC endpoint with the same sample data.
* reviewing gateway/direct timing ratios for the documented small and medium query pairs.

## Risks, Assumptions, and Open Decisions

* A live gateway plus Exasol Personal is required to learn actual unsupported PostgreSQL query families.
* The local environment might not have a JDK; the framework should still be check-in ready with a clear run script and docs.
* DBeaver and pgJDBC both use richer catalog surfaces than the current smoke coverage, so the first report may show many exploratory failures.
* Metabase behavior is partly inferred from public driver docs and JDBC patterns, so the corpus should be treated as representative, not exhaustive.
* Read-heavy latency benchmarks will mostly measure protocol and driver overhead unless they are paired with longer analytical queries.
* For fairness, gateway and direct Exasol benchmarks need logically equivalent query pairs, not identical SQL text.
