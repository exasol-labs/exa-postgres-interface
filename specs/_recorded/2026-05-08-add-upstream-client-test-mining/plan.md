# Plan: add-upstream-client-test-mining

## Summary

Investigation plan that evaluates two complementary strategies for systematically expanding gateway compatibility coverage of DBeaver and Metabase: mining SQL from upstream open-source code, and synthetic pgJDBC protocol probes. Delivers an ADR plus a small proof-of-concept (~5 mined probes per tool) wired into the existing harness; full corpora and the per-tool baseline-promotion machinery are deferred to follow-up plans.

## Domain Choice

This plan attaches its spec deltas to the existing `testing/client-compatibility-harness` feature. Rationale:

- The mined probes plug into the same harness, run under the same single-command entry point, and are reported alongside existing persona corpora.
- Provenance metadata, per-tool baseline classification, and license-aware corpus storage are all observable behaviors of the same harness, not a separate capability.
- Spawning a new feature (e.g., `upstream-test-corpus-provenance`) would fragment the harness contract across two specs without buying clarity.

If follow-up plans grow the per-tool corpora to the point where they warrant their own feature, that decision can be made later. For this investigation, the simplest correct attachment is the existing feature.

## Design

### Context

Real DBeaver and Metabase sessions surface gateway errors that the existing hand-curated probes miss. The investigation must answer whether mining upstream source for SQL is viable, how to handle Metabase's AGPL license, and what synthetic pgJDBC coverage should look like in follow-up plans.

- **Goals**
  - Decide between mining vs synthetic-pgJDBC vs both, with a concrete AGPL recommendation.
  - Demonstrate mining is tractable via a small POC.
  - Specify the per-tool baseline-promotion mechanism so follow-up plans can implement it.
- **Non-Goals**
  - Build the full DBeaver or Metabase corpus.
  - Implement the baseline-promotion machinery.
  - Build the synthetic-pgJDBC protocol coverage layer.
  - Automate re-mining as upstream evolves.

### Decision

See `adr-0001-upstream-client-test-mining.md` in this plan directory for the full Architecture Decision Record. Summary:

- Adopt mining as the primary near-term approach; synthetic pgJDBC as a follow-up.
- Isolate Metabase-derived probes under `tests/jdbc/upstream-mined/metabase/` with an AGPL notice covering only that directory; the gateway runtime artifact MUST NOT bundle that directory.
- DBeaver-derived probes live under `tests/jdbc/upstream-mined/dbeaver/` with attribution comments; no separate LICENSE file needed (Apache 2.0).
- Specify per-tool baseline-promotion via checked-in `tests/jdbc/baselines/<tool>.txt` files; defer implementation.

#### Architecture

```
                   scripts/run_jdbc_compatibility_suite.sh
                                  |
                                  v
                   tests/jdbc/PgJdbcCompatibilitySuite.java
                                  |
                  +---------------+----------------+
                  |               |                |
        existing corpus    upstream-mined     (future) synthetic
        (in-source)        loader              pgJDBC protocol
                                  |
                  +---------------+----------------+
                  |                                |
       tests/jdbc/upstream-mined/        tests/jdbc/upstream-mined/
       dbeaver/  (Apache 2.0,             metabase/ (AGPL-3.0,
       attribution comments)              isolated, NOT bundled
                                          into release artifact)
```

Each mined probe carries provenance fields: `upstreamProject`, `upstreamFile`, `upstreamSha`, `upstreamLicense`. The reporter prints these alongside the probe outcome.

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Provenance metadata on test probes | `QueryProbe` extension | Lets reviewers verify each mined probe traces to a specific upstream line. |
| License-isolated test directory | `tests/jdbc/upstream-mined/metabase/` | Conservative AGPL handling; reversible if legal posture changes. |
| Manual baseline-file promotion | `tests/jdbc/baselines/<tool>.txt` (specified, not implemented) | Decouples per-tool regression from global must-pass; matches the user's stated "pinned once it passes" model. |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|-------------------------|-----------|
| Investigation plan first, implementation follow-ups | One large plan covering everything | User explicitly chose ADR + POC; full corpora and baseline machinery follow once the approach is validated. |
| Mining + synthetic, both adopted | Pick one | Mining catches tool-specific SQL; synthetic catches protocol-level surfaces neither tool's source exposes as text. |
| AGPL isolation by directory | Treat SQL as non-copyrightable; abandon Metabase mining | Conservative, easy to relax later, removes ambiguity from the build. |
| POC ~5 probes per tool, all exploratory | Larger POC; smaller POC | Five is enough to demonstrate the tooling works without consuming the budget that belongs to follow-up plans. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| testing/client-compatibility-harness | CHANGED | `testing/client-compatibility-harness/spec.md` |

## Out of Scope

The following are explicitly out of scope for this plan:

- **Full DBeaver corpus** beyond the ~5 POC probes. Delivered by follow-up plan `add-dbeaver-mined-corpus`.
- **Full Metabase corpus** beyond the ~5 POC probes. Delivered by follow-up plan `add-metabase-mined-corpus`.
- **Synthetic pgJDBC protocol coverage** (extended-query round-trips, parameter-status, cursors, SQLSTATE). Specified in the ADR; delivered by follow-up plan `add-synthetic-pgjdbc-protocol-coverage`.
- **Per-tool baseline-promotion machinery.** The mechanism is specified in the ADR and the spec delta; implementation is delivered by follow-up plan `add-per-tool-baseline-promotion`.
- **Automated re-mining** as upstream tools update. Out of scope; manual re-mining is acceptable for now.
- **Live capture-replay** from running clients. Explicitly rejected by the user during the clarifying interview.
- **Running upstream test suites directly** against the gateway. Explicitly rejected by the user.

## Implementation Tasks

1. Author the ADR `specs/_plans/add-upstream-client-test-mining/adr-0001-upstream-client-test-mining.md` covering the mining vs synthetic-pgJDBC comparison, the AGPL handling decision, the per-tool baseline mechanism, and the follow-up plans required.
2. Author the spec delta `specs/_plans/add-upstream-client-test-mining/testing/client-compatibility-harness/spec.md` with the five new scenarios (provenance, license-aware storage, ADR existence, POC presence, baseline-promotion specification).
3. Extend `QueryProbe` in `tests/jdbc/PgJdbcCompatibilitySuite.java` with optional provenance fields (`upstreamProject`, `upstreamFile`, `upstreamSha`, `upstreamLicense`) defaulting to null for hand-curated probes. Update the reporter to print provenance fields for every mined probe outcome.
4. Create the directory `tests/jdbc/upstream-mined/dbeaver/` with a `README.md` documenting the upstream project, source paths, license (Apache 2.0), commit SHA pinned at mining time, and attribution.
5. Create the directory `tests/jdbc/upstream-mined/metabase/` with a `README.md` and a `LICENSE-AGPL.txt` declaring AGPL-3.0 inheritance for that directory only. Document that the gateway runtime artifact MUST NOT bundle this directory.
6. Mine ~5 SQL strings from real DBeaver upstream files (candidates: `PostgreUtils.java`, `PostgreDialect.java`, the metadata-cache classes under `org.jkiss.dbeaver.ext.postgresql`); add them as `EXPLORATORY` probes with full provenance comments referencing the upstream file path and commit SHA. [expert]
7. Mine ~5 SQL strings from real Metabase upstream files (candidates: `src/metabase/driver/postgres.clj`, the `metabase/driver/sql_jdbc/sync` namespace) into the AGPL-isolated directory; add them as `EXPLORATORY` probes with full provenance and a comment pointer to the AGPL notice. Configure the test loader so these probes load only at test-execution time and never enter the release artifact. [expert]
8. Update `scripts/run_jdbc_compatibility_suite.sh` (or its compile step) so the new mined-probe sources are picked up by the existing single-command entry point with no extra steps for the operator.
9. Update the harness documentation (in-source or under `docs/`) to identify which upstream tools informed each persona, the provenance contract, the AGPL-isolation rule, and the planned per-tool baseline-promotion mechanism.
10. Run `cargo test`, `cargo fmt --check`, and the compatibility-suite single-command entry point against a configured gateway to confirm the mined probes execute and the report classifies them correctly.

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A (specs and ADR) | Tasks 1, 2 |
| Group B (harness plumbing) | Tasks 3, 4, 5 |
| Group C (mining) | Tasks 6, 7 |
| Group D (entry point and docs) | Tasks 8, 9 |
| Group E (verification) | Task 10 |

Sequential dependencies:
- Group A → Group B (deltas inform the harness change shape).
- Group B → Group C (probes need the provenance fields and the directory structure to land in).
- Group C → Group D (run script update references the new sources).
- Group D → Group E (verification runs after everything is wired).

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| (none) | n/a | This investigation plan adds new behavior; nothing existing becomes obsolete. The hand-curated DBeaver/Metabase probes remain valid until follow-up plans replace them with mined ones. |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| Mined probes carry upstream provenance metadata | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` | mined probe outcomes printed by `Reporter` MUST include the provenance fields; verified by running the suite and inspecting the report (asserted by a new `ProvenanceReportTest` block in the suite that fails if any mined probe lacks the four provenance fields) |
| Mined corpora are stored according to upstream license | Integration | `tests/jdbc/upstream-mined/metabase/README.md` and `tests/jdbc/upstream-mined/metabase/LICENSE-AGPL.txt` exist; `cargo build --release` artifact MUST NOT contain Metabase-derived files | new shell check `tests/jdbc/check_release_artifact_clean.sh` invoked from the suite that greps the release artifact for any path under `tests/jdbc/upstream-mined/metabase/` |
| Investigation produces an ADR comparing mining and synthetic-pgJDBC approaches | Integration | `specs/_plans/add-upstream-client-test-mining/adr-0001-upstream-client-test-mining.md` exists and is linked from `plan.md` | `speq plan validate add-upstream-client-test-mining` plus a CI check that the ADR file exists |
| Proof-of-concept demonstrates mining is tractable | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` running under `scripts/run_jdbc_compatibility_suite.sh` | running the script reports at least 5 DBeaver-mined and 5 Metabase-mined probes with provenance, classified `EXPLORATORY` |
| Per-tool baseline promotion mechanism is specified | Integration | `specs/_plans/add-upstream-client-test-mining/adr-0001-upstream-client-test-mining.md` and `specs/_plans/add-upstream-client-test-mining/testing/client-compatibility-harness/spec.md` | `speq plan validate add-upstream-client-test-mining` plus reviewer confirms the mechanism is described concretely (file path, file format, classification axis) |

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| testing/client-compatibility-harness | `scripts/run_jdbc_compatibility_suite.sh 'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' sys 'EXASOL_PASSWORD' --personas=dbeaver,metabase` | Report includes at least 5 DBeaver-mined and 5 Metabase-mined probes; each prints `upstreamProject`, `upstreamFile`, `upstreamSha`, `upstreamLicense`; failures appear as `EXPLORATORY`, never as must-pass. |
| testing/client-compatibility-harness (release-artifact check) | `cargo build --release && tests/jdbc/check_release_artifact_clean.sh target/release/` | Exit 0; no path under `tests/jdbc/upstream-mined/metabase/` appears in any release artifact. |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build --release` | Exit 0 |
| Test | `cargo test` | 0 failures |
| Format | `cargo fmt --check` | No changes |
| Spec validation | `speq plan validate add-upstream-client-test-mining` | pass |
| Compatibility suite (manual, against deployed gateway) | `scripts/run_jdbc_compatibility_suite.sh ...` | Mined probes execute and report provenance |
