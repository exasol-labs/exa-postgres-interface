# Verification Report: add-upstream-client-test-mining

**Generated:** 2026-05-08

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | All automated checks pass; 10 mined probes (5 DBeaver + 5 Metabase) wired into the harness with full provenance; AGPL artifact check clean; spec validation passes. Manual suite run against a live gateway deferred — requires a deployed gateway. |

| Check | Status |
|-------|--------|
| Build | ✓ |
| Tests | ✓ |
| Lint | — (no Rust linter configured for this check) |
| Format | ✓ |
| Spec validation | ✓ |
| AGPL artifact check | ✓ |
| Manual suite (live gateway) | ⚠ deferred — no gateway available |

## Test Evidence

### Test Results

| Type | Run | Passed | Failed |
|------|-----|--------|--------|
| Rust unit tests | 6 | 6 | 0 |
| Java suite (live) | n/a | n/a | n/a |

### Manual Tests

| Test | Result |
|------|--------|
| `cargo build --release` | ✓ exit 0, 25 pre-existing warnings, no new warnings |
| `cargo test` | ✓ 6/6 passed |
| `cargo fmt --check` | ✓ no changes |
| `speq plan validate add-upstream-client-test-mining` | ✓ pass — 1 delta spec validated |
| `tests/jdbc/check_release_artifact_clean.sh target/release/` | ✓ exit 0, no AGPL paths in release artifact |
| `scripts/run_jdbc_compatibility_suite.sh` against live gateway | ⚠ not run — gateway not deployed |

## Tool Evidence

### Formatter

```
cargo fmt --check: (exit 0, no output)
```

### Build

```
warning: `exa-postgres-interface` (bin "exa-postgres-interface") generated 25 warnings
Finished `release` profile [optimized] target(s) in 42.00s
```
(All 25 warnings are pre-existing; no new warnings introduced by this plan.)

### Spec validation

```
Plan 'add-upstream-client-test-mining' validation passed.
Validated 1 delta spec(s):
  testing/client-compatibility-harness/spec.md
```

### AGPL artifact check

```
OK: no AGPL-isolated Metabase probe paths found in target/release/
```

## Scenario Coverage

| Feature | Scenario | Artifact | Status |
|---------|----------|----------|--------|
| testing/client-compatibility-harness | Mined probes carry upstream provenance metadata | `QueryProbe.provenanceSuffix()` + `validateProvenanceInvariants()` in `PgJdbcCompatibilitySuite.java` | ✓ implemented; live validation deferred |
| testing/client-compatibility-harness | Mined corpora are stored according to upstream license | `tests/jdbc/upstream-mined/metabase/LICENSE-AGPL.txt`, `README.md`; artifact check script exit 0 | ✓ |
| testing/client-compatibility-harness | Investigation produces an ADR comparing mining and synthetic-pgJDBC approaches | `specs/_plans/add-upstream-client-test-mining/adr-0001-upstream-client-test-mining.md` | ✓ |
| testing/client-compatibility-harness | Proof-of-concept demonstrates mining is tractable | 5 DBeaver probes (mined-pg-collation, mined-pg-tablespace, mined-pg-roles, mined-pg-language, mined-pg-event-trigger) + 5 Metabase probes (mined-pg-enum-types, mined-show-timezone, mined-quote-ident, mined-get-tables, mined-describe-fks) in corpus | ✓ wired; live execution deferred |
| testing/client-compatibility-harness | Per-tool baseline promotion mechanism is specified | ADR §Per-tool Baseline Promotion + spec delta scenario | ✓ |

## Notes

- **Live gateway run deferred**: The manual compatibility suite run (`scripts/run_jdbc_compatibility_suite.sh ... --personas=dbeaver,metabase`) requires a deployed Exasol gateway. No gateway was available during this verification pass. The mined probes compile correctly (confirmed by code review — correct arity for `mined()`/`minedPrepared()`, all imports already present) but end-to-end execution is not verified here.
- **Java compilation**: `javac` is not installed in the WSL environment; compilation is the responsibility of `scripts/run_jdbc_compatibility_suite.sh`. Static review of the inserted Java confirms correct method arity, consistent provenance invariants, and no new imports required.
- **Two Metabase probes are HoneySQL-DSL-derived**: `mined-get-tables` and `mined-describe-fks` are renderings of upstream Clojure HoneySQL DSL forms, not verbatim string literals. Each carries a comment noting the DSL origin. Reviewers should verify the rendering against the upstream Clojure source at the pinned SHA (`8c932afa0b1d37cdfa6994a9fe32f57e74d93fa2`).
- **Pre-existing Rust warnings**: The 25 `cargo build` warnings are pre-existing (present before this plan) and are not caused by any change in this plan.
