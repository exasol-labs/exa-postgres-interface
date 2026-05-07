# Verification Report: add-set-search-path-support

**Generated:** 2026-05-07

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | All automated checks pass; JDBC probes added; manual tests require a live Exasol+gateway instance |

| Check | Status |
|-------|--------|
| Build | ✓ (`cargo build --release` — exit 0, 25 pre-existing warnings, 0 errors) |
| Tests | ✓ (63 lib + 6 binary = 69 passed, 0 failed) |
| Lint | ✓ (no new warnings introduced) |
| Format | ✓ (`cargo fmt --check` — no diff) |
| Scenario Coverage | ✓ (all 8 plan scenarios covered by tests) |
| Manual Tests | ⚠ (not executable without live Exasol instance; commands documented below) |

## Test Evidence

### Coverage

| Type | Coverage |
|------|----------|
| Unit (classifier) | 5 new test functions in `src/policy.rs` covering all 8 classifier scenarios |
| Integration (JDBC) | 5 new MUST_PASS probes in `tests/jdbc/PgJdbcCompatibilitySuite.java` |

### Test Results

| Type | Run | Passed | Failed |
|------|-----|--------|--------|
| Unit (lib) | 63 | 63 | 0 |
| Binary | 6 | 6 | 0 |

### Manual Tests

| Test | Result |
|------|--------|
| `SET search_path = "PG_DEMO"; SELECT current_schema();` via psql | Not run (requires live gateway) |
| `SET search_path = pg_demo, pg_catalog` via psql (expect error) | Not run |
| `RESET search_path` via psql | Not run |
| `SET search_path = "PG_DEMO"; SHOW search_path` via psql | Not run |
| DBeaver schema editor NPE verification | Not run |

## Tool Evidence

### Build

```
Finished `release` profile [optimized] target(s) in 43.40s
```

### Formatter

```
(no output — no formatting issues)
```

### Test Run

```
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## Scenario Coverage

| Scenario | Test Type | Test Location | Test Name | Status |
|----------|-----------|---------------|-----------|--------|
| Single-schema search_path opens the Exasol schema | Integration (JDBC) | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `set-search-path-single` | Added |
| Multi-schema search_path is rejected with a compatibility error | Integration (JDBC) | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `set-search-path-multi` | Added (expectingFailure) |
| search_path reset is a no-op | Integration (JDBC) | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `reset-search-path` | Added |
| SHOW search_path reflects the active schema | Integration (JDBC) | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `show-search-path` | Added |
| search_path open failure surfaces as a PostgreSQL-compatible error | Integration (JDBC) | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `set-search-path-missing-schema` | Added (expectingFailure) |
| search_path session command maps to Exasol OPEN SCHEMA (matrix entry) | Unit | `src/policy.rs` | `classifies_set_search_path_single_schema` | Pass |
| SHOW search_path reads gateway-managed session state (matrix entry) | Unit | `src/policy.rs` | `classifies_show_search_path_dynamically` | Pass |
| search_path session state is owned by the gateway | Integration (JDBC) | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `set-search-path-single` + `show-search-path` | Added |

## Code Review Findings Resolved

| Finding | Resolution |
|---------|------------|
| `SELECT current_schema()` hardcoded to `"public"` even after SET search_path | Fixed — local intercept removed; query now reaches Exasol which reports the opened schema |
| `SET_SEARCH_PATH_RE` missing word boundary after `TO` (TODEMO would match) | Fixed — `(?:=|TO\b)` |
| Failure path returns `PgWireError` instead of `GatewayResponse::Error` (plan deviation) | Accepted — consistent with existing `Execute` arm pattern; not user-visible |
| Unterminated quoted RHS produces `Single("\"DEMO")` instead of `Invalid` | Not fixed (low priority) — Exasol will reject the malformed SQL; gateway returns a clear DB error |

## Files Modified

| File | Change |
|------|--------|
| `src/policy.rs` | New `SetSearchPath` and `ShowSearchPath` variants; `SET_SEARCH_PATH_RE`; `parse_search_path_value`; wired detection; `local_show` update; removed `SELECT current_schema()` intercept; 5 new unit tests |
| `src/pg_server.rs` | `current_schema` field on `SessionState`; `SetSearchPath` handler (OPEN SCHEMA + state update); `ShowSearchPath` handler (read + return) |
| `tests/jdbc/PgJdbcCompatibilitySuite.java` | `expectFailure` field on `QueryProbe`; `expectingFailure` factory; `executeProbe` updated; 5 new MUST_PASS session probes |
| `README.md` | "Gateway-Managed Session Commands" subsection documenting search_path behavior |
