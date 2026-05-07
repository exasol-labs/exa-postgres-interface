# Tasks: add-set-search-path-support

## Group A — Classifier (Tasks 1, 2, 3)
- [x] 1.1 Add `StatementPlan::SetSearchPath { schema: String }` variant
- [x] 1.2 Add `StatementPlan::ShowSearchPath` variant (no payload)
- [x] 2.1 Define `SET_SEARCH_PATH_RE` regex [expert]
- [x] 2.2 Add `parse_search_path_value(rhs: &str) -> SearchPathTarget` helper [expert]
- [x] 2.3 Wire search_path detection ahead of `is_safe_set`
- [x] 2.4 Add `RESET search_path` regression test
- [x] 2.5 Change `local_show` to return `ShowSearchPath` for `search_path` key
- [x] 3.1 Unit test: `SET search_path = "DEMO_FINANCE"` → `SetSearchPath { schema: "DEMO_FINANCE" }`
- [x] 3.2 Unit test: `SET search_path TO demo_finance` → `SetSearchPath { schema: "demo_finance" }`
- [x] 3.3 Unit test: `SET SESSION search_path = 'pg_demo'` → `SetSearchPath { schema: "pg_demo" }`
- [x] 3.4 Unit test: `SET search_path = pg_demo, pg_catalog` → `Reject`
- [x] 3.5 Unit test: `SET search_path = DEFAULT` → `ClientSet`
- [x] 3.6 Unit test: `RESET search_path` → `ClientSet`
- [x] 3.7 Unit test: `SHOW search_path` → `ShowSearchPath`
- [x] 3.8 Unit test: other SET statements still classify as `ClientSet` (regression)

## Group B — Handler (Tasks 4, 5, 6)
- [x] 4.1 Add `current_schema: Mutex<Option<String>>` field to `SessionState`
- [x] 4.2 Initialize `current_schema` to `None` in `connect_exasol`
- [x] 5.1 Build `OPEN SCHEMA "<schema>"` SQL with double-quoting and escape
- [x] 5.2 Run the SQL via `task::spawn_blocking` + `state.exasol.lock()` pattern
- [x] 5.3 On success: set `state.current_schema`, return `GatewayResponse::Execution { command: "SET", rows: None }`
- [x] 5.4 On failure: do NOT mutate `state.current_schema`; return `GatewayResponse::Error`
- [x] 6.1 Read `state.current_schema` (`unwrap_or("public")`) for `ShowSearchPath`
- [x] 6.2 Return `GatewayResponse::Query` with `search_path` column

## Group C — Verification (Tasks 7, 8)
- [x] 7.1 Integration test: `classifies_set_search_path_single_schema`
- [x] 7.2 Integration test: `rejects_set_search_path_multi_schema`
- [x] 7.3 Integration test: `handles_search_path_reset_and_default`
- [x] 7.4 Integration test: `classifies_show_search_path_dynamically`
- [x] 8.1 Update JDBC compatibility suite with `set-search-path-single` probe
- [x] 8.2 Add `set-search-path-multi` probe (MUST_PASS)
- [x] 8.3 Add `reset-search-path` probe (MUST_PASS)
- [x] 8.4 Add `show-search-path` probe (MUST_PASS)
- [x] 8.5 Add `set-search-path-missing-schema` probe (MUST_PASS)

## Group D — Docs (Task 9)
- [x] 9.1 Document new behavior in `README.md`
