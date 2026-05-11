# ADR-0001: Expanding Client Compatibility Coverage Through Upstream Source Mining and Synthetic pgJDBC Probes

Date: 2026-05-08
Status: Proposed (investigation outcome of the `add-upstream-client-test-mining` plan)
Format: Nygard ADR

## Context

The PostgreSQL gateway ships with a JDBC compatibility harness (see `testing/client-compatibility-harness`) that includes:

- An exhaustive `DatabaseMetaData` sweep via reflection.
- Persona-organized SQL probes including a small set of hand-curated DBeaver and Metabase queries (currently 5-6 each, all `EXPLORATORY`).
- A single-command entry point (`scripts/run_jdbc_compatibility_suite.sh`).

In practice, **real DBeaver and Metabase sessions still surface gateway errors and unexpected behaviors that the harness does not catch**. The reason is that the existing DBeaver/Metabase probes were extracted by guesswork rather than from the tools' actual code paths, so they cover only a tiny fraction of the SQL these tools issue against PostgreSQL.

The team needs a strategy to systematically expand coverage that:

1. Leverages the open-source nature of DBeaver (Apache 2.0) and Metabase (AGPL-3.0).
2. Avoids brittle dependence on specific upstream releases.
3. Respects upstream licenses, in particular Metabase's AGPL.
4. Keeps the gateway runtime artifact free of copyleft contamination.

This ADR records the decision reached during the investigation phase. Implementation of the full corpora and the per-tool baseline-promotion machinery is intentionally deferred to follow-up plans.

## Goals

- Enumerate the considered approaches for expanding client coverage.
- Recommend a primary approach and a complementary one.
- Resolve the AGPL question for Metabase concretely, not by hand-wave.
- Identify the minimal proof-of-concept that demonstrates the recommended approach is tractable.
- Identify follow-up plans required for full implementation.

## Non-Goals

- Building the full per-tool corpora in this plan.
- Implementing the per-tool baseline-promotion machinery in this plan.
- Automating re-mining as upstream tools update.
- Live wire-traffic capture from running clients.
- Running upstream test suites directly against the gateway.

## Considered Options

### Option 1 - Mine SQL from upstream source code

Extract the actual SQL strings issued by DBeaver (Java) and Metabase (Clojure) against PostgreSQL by reading their open-source repositories.

Concrete upstream targets identified during investigation:

- **DBeaver**: `plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/` — in particular `PostgreUtils.java`, `PostgreDialect.java`, and the metadata-cache classes that hold the literal SQL strings DBeaver issues against `pg_catalog.pg_namespace`, `pg_class`, `pg_attribute`, `pg_constraint`, `pg_index`, and `pg_description`. License: Apache 2.0.
- **Metabase**: `src/metabase/driver/postgres.clj` plus the shared `metabase/driver/sql_jdbc/sync` namespace, which hold metadata-sync queries and the `LIMIT 0` / `LIMIT 1` query-metadata patterns. License: AGPL-3.0.

Strengths:
- Captures the exact statements the tools issue, not guesses.
- Reproducible: each probe carries an upstream file path and commit SHA, so reviewers can verify.
- Extends naturally as upstream evolves (manual re-mining; automation is out of scope).

Weaknesses:
- Per-tool effort is non-trivial; a full corpus is a follow-up plan, not this one.
- AGPL-3.0 (Metabase) raises a copyleft question for any derived material.

### Option 2 - Synthetic from pgJDBC + driver protocols

Expand the existing pgJDBC-driven sweep so any tool using pgJDBC is covered behaviorally, independent of which UI tool is in use.

Behavioral surfaces that the synthetic approach SHOULD cover in follow-up plans:

- **Extended query protocol round-trips**: `Parse` / `Bind` / `Describe` / `Execute` / `Sync` interleaved with parameter binds.
- **Parameter-status messages**: the gateway must emit (or pass through) the `ParameterStatus` set that pgJDBC reads at startup and after `SET` statements (`server_version`, `client_encoding`, `DateStyle`, `TimeZone`, `IntervalStyle`, `integer_datetimes`, `is_superuser`, `session_authorization`, `standard_conforming_strings`).
- **Prepared-statement describe round-trips**: server-prepared statements with explicit `Describe` after `Parse` to obtain `ParameterDescription` and `RowDescription` before `Bind`.
- **Server-side cursors**: `setFetchSize(n)` switches pgJDBC into cursor mode; the gateway must support `Execute` with non-zero row limit, `PortalSuspended`, and re-`Execute`.
- **Error class-codes**: SQLSTATE values pgJDBC depends on for retry/abort logic (`08006`, `25P02`, `40001`, `42P01`, `42501`, `XX000`).
- **`COPY` start/stop sequences** (out-of-scope for v1 but enumerated for follow-up).

Strengths:
- License-clean (pgJDBC tests are BSD-2-Clause).
- Tool-agnostic: covers any pgJDBC-using client (DBeaver, Metabase via pgJDBC, DbVisualizer, JetBrains, etc.) at the protocol layer.
- pgJDBC's own test suite (`pgjdbc/pgjdbc/src/test/java/org/postgresql/test/jdbc2`) is a rich source of patterns we can mirror without copying.

Weaknesses:
- Misses tool-specific SQL that runs above pgJDBC (DBeaver pg_catalog joins, Metabase information_schema patterns).
- Building protocol-level tests requires more harness plumbing than appending SQL strings to the existing probe corpus.

### Option 3 - Capture-replay from a running client (rejected)

User-rejected during the clarifying interview. Brittle (requires bringing up DBeaver/Metabase, harder to reproduce in CI, ties tests to a UI session).

### Option 4 - Run upstream test suites directly (rejected)

User-rejected. Pulling DBeaver's or Metabase's test infrastructure into our build is heavyweight, drags in their build systems, and inherits AGPL transitively for Metabase.

## Decision

Adopt **both Option 1 and Option 2 as complementary approaches**, with the following layering:

1. **Upstream-source mining (Option 1) is the primary near-term approach.** It directly addresses the user's complaint that hand-curated probes miss real failures.
2. **Synthetic pgJDBC protocol coverage (Option 2) is the second-phase approach** for surfaces the mined SQL cannot reach (extended-query round-trips, parameter-status, cursors, error class-codes).

Both approaches feed into the same harness and are reported in the same single-command run.

### AGPL Handling for Metabase (concrete decision, not hand-waved)

The investigation considered three sub-options for the AGPL question:

- **(a) Treat short SQL fragments as non-copyrightable facts.** Plausible under the scenes-à-faire doctrine for SQL that targets PostgreSQL's standardized `pg_catalog` and `information_schema`, but legally unsettled. Risk: relies on a defense rather than a license-clean path.
- **(b) Keep mined Metabase corpus in a separate, AGPL-compatible artifact.** Concrete, conservative, and removes ambiguity.
- **(c) Abandon direct mining for Metabase and replace with capture-from-running-Metabase.** Rejected for the same reasons as Option 3 above.

**The decision is (b): isolate Metabase-derived material.** Specifically:

- Metabase-derived probes SHALL live under `tests/jdbc/upstream-mined/metabase/` (a new directory created in this plan's POC).
- That directory SHALL contain a `LICENSE-AGPL.txt` and a `README.md` declaring AGPL-3.0 inheritance for the contents of that directory only.
- The harness SHALL load Metabase-derived probes at test time from that directory; the gateway runtime artifact (`cargo build --release` output) MUST NOT bundle any file from that directory.
- The repository's primary license is unaffected for non-Metabase code paths.
- DBeaver-derived probes (Apache 2.0) live under `tests/jdbc/upstream-mined/dbeaver/` with attribution comments; they MAY be co-located with the rest of the test code without a separate LICENSE file because Apache 2.0 is compatible with the repository's existing license posture.

This decision is conservative. If a future legal review concludes that the SQL strings themselves are non-copyrightable facts, the isolation MAY be relaxed; reversing it later is cheaper than retrofitting isolation after the fact.

### Per-tool Baseline Promotion (mechanism only, no implementation)

The investigation specifies the mechanism so follow-up plans can implement it. The recommended shape:

- A checked-in `tests/jdbc/baselines/<tool>.txt` file lists the IDs of probes pinned as regressions for that tool.
- The reporter classifies a probe outcome as one of: `must-pass-failure`, `tool-baseline-failure` (per-tool pinned probe failed), `exploratory-failure`, or `pass`.
- Promotion is a manual step: a maintainer adds the probe ID to the appropriate `<tool>.txt` after seeing the probe pass in CI for that tool.
- The current `Expectation` enum (`MUST_PASS` / `EXPLORATORY`) is preserved; tool-baseline membership is an orthogonal axis loaded from the baseline files.

Implementation of this mechanism is deferred to a follow-up plan.

## Consequences

| Decision | Alternatives Considered | Rationale |
|----------|-------------------------|-----------|
| Mining is the primary near-term approach | Synthetic-only; capture-replay; running upstream suites | Mining directly addresses the gap (real tool SQL) the user flagged; synthetic is complementary, not a substitute. |
| Synthetic pgJDBC coverage is a follow-up plan | Build it now alongside mining | Synthetic requires more harness plumbing; the POC budget is better spent proving mining is tractable first. |
| Metabase-derived probes are isolated under `tests/jdbc/upstream-mined/metabase/` with AGPL notice | Treat SQL as non-copyrightable; abandon Metabase mining | Conservative, license-clean, easy to relax later if legal review changes the analysis. |
| DBeaver-derived probes co-locate under `tests/jdbc/upstream-mined/dbeaver/` with attribution | Same isolation as Metabase | Apache 2.0 is permissive; isolation buys nothing here and adds friction. |
| Per-tool baseline promotion is specified, not implemented | Implement now | Out of scope for this investigation plan; specifying the mechanism unblocks the follow-up plan. |
| Automated re-mining is out of scope | Build a re-miner now | Freshness concern is real but separable; manual re-mining is acceptable until the corpus stabilizes. |

## Follow-up Plans Required

1. **`add-dbeaver-mined-corpus`** — full DBeaver-derived corpus beyond the POC; runs against a deployed gateway and reports per-feature gaps.
2. **`add-metabase-mined-corpus`** — full Metabase-derived corpus under the AGPL-isolated directory.
3. **`add-synthetic-pgjdbc-protocol-coverage`** — implements the extended-query / parameter-status / cursor / SQLSTATE coverage enumerated above.
4. **`add-per-tool-baseline-promotion`** — implements the `baselines/<tool>.txt` mechanism in the reporter.

## References

- DBeaver license (Apache 2.0): https://github.com/dbeaver/dbeaver/blob/devel/LICENSE.md
- DBeaver PostgreSQL plugin: https://github.com/dbeaver/dbeaver/tree/devel/plugins/org.jkiss.dbeaver.ext.postgresql
- Metabase license (AGPL-3.0): https://github.com/metabase/metabase/blob/master/LICENSE-AGPL.txt
- Metabase Postgres driver: https://github.com/metabase/metabase/blob/master/src/metabase/driver/postgres.clj
- pgJDBC test suite layout: https://github.com/pgjdbc/pgjdbc/blob/master/TESTING.md
- Existing harness spec: `specs/testing/client-compatibility-harness/spec.md`
- Existing harness implementation: `tests/jdbc/PgJdbcCompatibilitySuite.java`
- Existing entry point: `scripts/run_jdbc_compatibility_suite.sh`
