# Plan: add-set-search-path-support

## Summary

Detect `SET search_path` in the gateway's statement classifier and translate single-schema assignments into Exasol `OPEN SCHEMA` while tracking the active schema in per-session state so PostgreSQL clients (DBeaver, psql, JDBC) that depend on schema context work without raising NPEs in their drivers.

## Design

### Context

PostgreSQL clients use `SET search_path = "<schema>"` to scope subsequent statements to a single schema. The gateway currently treats every `SET <name> = <value>` as a no-op `ClientSet` (success without forwarding anything to Exasol). The Exasol session is left without an active schema, so `current_schema()` returns NULL and JDBC drivers crash with `Cannot invoke "java.lang.CharSequence.toString()" because "s" is null` when they query schema-dependent metadata.

Exasol's session-level equivalent of "select an active schema" is `OPEN SCHEMA <name>`. Exasol does not support multiple simultaneously active schemas and does not document a "close schema" statement, so multi-schema search paths and `RESET search_path` need explicit gateway policy decisions rather than direct mappings.

- **Goals**
  - Translate single-schema `SET search_path = <schema>` into `OPEN SCHEMA <schema>` against Exasol.
  - Track the active schema in `SessionState` so `SHOW search_path` reports it dynamically.
  - Reject multi-schema search paths with a clear PostgreSQL-compatible error so clients stop sending the unsupported form.
  - Accept `RESET search_path` and `SET search_path = DEFAULT` as no-ops because Exasol has no "close schema" verb.
  - Keep the response shape PostgreSQL-compatible (command tag `SET`).

- **Non-Goals**
  - Multi-schema search path semantics. Exasol cannot honor them, so emulating PostgreSQL's name-resolution order across multiple schemas is out of scope.
  - Schema-search fallback semantics for `pg_catalog`. The PostgreSQL implicit search of `pg_catalog` is provided through the gateway's existing catalog compatibility, not through `search_path`.
  - Translating `current_schemas()` (plural) — only the singular `current_schema()` is in the trigger sequence, and Exasol provides a correct value once `OPEN SCHEMA` has been issued.
  - Per-statement schema scoping (e.g., `SET LOCAL search_path`). The MVP applies search_path at session scope only.

### Decision

Add a new `StatementPlan::SetSearchPath { schema }` variant rather than reusing `StatementPlan::Execute`. The dispatcher in `pg_server.rs` then issues `OPEN SCHEMA <schema>` against Exasol, updates session-tracked active-schema state, and returns the PostgreSQL `SET` command tag (not `OPEN SCHEMA`). Reusing `Execute` would either leak an Exasol command tag back to the client or force `Execute` to learn about state mutation — both worse than a small, explicit variant.

For `SHOW search_path`, classification produces a new `StatementPlan::ShowSearchPath` (no resolved value at parse time). The handler in `pg_server.rs` reads the active schema from `SessionState.current_schema` at execution time. Existing `local_show()` keeps handling the static keys (`server_version`, `client_encoding`, etc.).

#### Architecture

```
classify_statement(sql)
  ├─ SET search_path = <single>   → SetSearchPath { schema }
  ├─ SET search_path = a, b, ...  → Reject (compatibility error)
  ├─ SET search_path = DEFAULT    → ClientSet (no-op)
  ├─ RESET search_path            → ClientSet (no-op)
  ├─ SHOW search_path             → ShowSearchPath
  ├─ ... (existing variants unchanged)

pg_server::execute_statement()
  ├─ SetSearchPath { schema }
  │     └─ session.execute("OPEN SCHEMA \"<schema>\"")
  │     └─ state.current_schema = Some(schema)
  │     └─ GatewayResponse::Execution { command: "SET", rows: None }
  ├─ ShowSearchPath
  │     └─ value = state.current_schema.unwrap_or("public")
  │     └─ GatewayResponse::Query { columns: ["search_path"], rows: [[value]] }
  └─ ... (existing variants unchanged)
```

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Dedicated plan variant | `StatementPlan::SetSearchPath` | Keeps Execute's command-tag invariant clean; localizes side-effect (mutating SessionState) to one handler arm |
| Late-binding SHOW | `StatementPlan::ShowSearchPath` resolves value in `pg_server.rs`, not `policy.rs` | `policy.rs` has no `SessionState` reference and stays a pure classifier |
| Identifier extraction helper | `parse_search_path_value(rhs) -> Option<SearchPathTarget>` | Single owner for quoting rules (double, single, unquoted), comma counting, and `DEFAULT` detection |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| New `SetSearchPath` variant | Reuse `Execute { command: "SET", row_count: Omit }` after rewriting the SQL string to `OPEN SCHEMA` | `Execute` has no hook to mutate `SessionState.current_schema`. Threading state mutation into the generic Execute arm complicates every other Execute caller. A dedicated variant pays its way the first time. |
| Late-binding `SHOW search_path` via `ShowSearchPath` | Extend `local_show()` to resolve dynamically by passing `&SessionState` into `classify_statement` | `classify_statement` is currently a pure function over SQL text. Threading session state through it inverts the layer boundary and forces every call site to acquire the mutex. |
| Reject multi-schema with compatibility error | Silently keep the first schema; silently no-op | Silent behavior hides material semantic differences from PostgreSQL clients; the project's mission explicitly forbids that. A clear error keeps the boundary visible. |
| `RESET search_path` is a no-op | Reject with an error; track a "no schema open" sentinel | Exasol has no "close schema" verb. Returning success matches PostgreSQL's user-visible contract for the simple case and avoids breaking clients that issue `RESET` defensively. |
| Quote schema identifier as `OPEN SCHEMA "<name>"` | Pass through the literal identifier untouched | DBeaver sends `SET search_path = "DEMO_FINANCE"` with double quotes preserved. Exasol's `OPEN SCHEMA` accepts a quoted identifier. Always wrapping the resolved name in double quotes preserves case and avoids reserved-word collisions. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| protocol/read-write-statement-path | CHANGED | `protocol/read-write-statement-path/spec.md` |
| sql/postgres-to-exasol-compatibility | CHANGED | `sql/postgres-to-exasol-compatibility/spec.md` |
| operations/gateway-owned-translation | CHANGED | `operations/gateway-owned-translation/spec.md` |

## Implementation Tasks

1. **Extend `StatementPlan` with new variants in `src/policy.rs`**
   1.1 Add `StatementPlan::SetSearchPath { schema: String }`.
   1.2 Add `StatementPlan::ShowSearchPath` (no payload — value is bound by the handler).

2. **Detect search_path in `classify_statement`**
   2.1 Define a `SET_SEARCH_PATH_RE` that matches `SET [SESSION] search_path` (case-insensitive) followed by `=` or `TO` and captures the right-hand side. [expert]
   2.2 Add a `parse_search_path_value(rhs: &str) -> SearchPathTarget` helper that returns one of `Single(String)`, `Default`, `Multi`, or `Invalid`. The helper MUST handle double-quoted (`"DEMO_FINANCE"`), single-quoted (`'demo_finance'`), and bare (`DEMO_FINANCE`) identifiers; treat `DEFAULT` (case-insensitive, unquoted) as `Default`; treat any comma at top level as `Multi`. [expert]
   2.3 Wire the detection ahead of `is_safe_set` so search_path no longer falls through to the generic `ClientSet` path. Single → `SetSearchPath`; Default → `ClientSet`; Multi → `Reject` with a compatibility error; Invalid → `Reject` with a syntax error.
   2.4 Add `RESET search_path` handling: when `RESET_RE` matches and the captured name is `search_path` (case-insensitive), return `ClientSet`. The existing fall-through already returns `ClientSet`, so confirm with a regression test rather than changing logic.
   2.5 Add `SHOW search_path` detection: change `local_show` so the `search_path` key returns `Some(StatementPlan::ShowSearchPath)` instead of `Some(StatementPlan::ClientShow { ... value: "public" })`.

3. **Add unit tests for the classifier in `src/policy.rs`**
   3.1 `SET search_path = "DEMO_FINANCE"` → `SetSearchPath { schema: "DEMO_FINANCE" }`.
   3.2 `SET search_path TO demo_finance` → `SetSearchPath { schema: "demo_finance" }`.
   3.3 `SET SESSION search_path = 'pg_demo'` → `SetSearchPath { schema: "pg_demo" }`.
   3.4 `SET search_path = pg_demo, pg_catalog` → `Reject` with message mentioning single-schema only.
   3.5 `SET search_path = DEFAULT` → `ClientSet`.
   3.6 `RESET search_path` → `ClientSet`.
   3.7 `SHOW search_path` → `ShowSearchPath`.
   3.8 Other `SET <name> = ...` statements still classify as `ClientSet` (regression — `application_name`, `extra_float_digits`, `SESSION CHARACTERISTICS`).

4. **Track active schema in `SessionState` (`src/pg_server.rs`)**
   4.1 Add `current_schema: Mutex<Option<String>>` field to `SessionState`.
   4.2 Initialize it to `None` in `connect_exasol`.

5. **Handle `SetSearchPath` in `execute_statement`**
   5.1 Acquire `SessionState`. Build the SQL `OPEN SCHEMA "<schema>"`, double-quoting the schema and escaping any embedded `"` by doubling.
   5.2 Run the SQL via the same `task::spawn_blocking` + `state.exasol.lock()` pattern used by `Execute`.
   5.3 On success, set `state.current_schema` to `Some(schema)` and return `GatewayResponse::Execution { command: "SET", rows: None }`.
   5.4 On failure, do NOT mutate `state.current_schema`; return a `GatewayResponse::Error` mapped through `map_exasol_execution_error`.

6. **Handle `ShowSearchPath` in `execute_statement`**
   6.1 Read `state.current_schema` (`unwrap_or("public")`).
   6.2 Return `GatewayResponse::Query { columns: vec!["search_path".into()], rows: vec![vec![Some(value)]] }`.

7. **Add Rust integration test for the policy classifier**
   7.1 Add `#[test] fn classifies_set_search_path_single_schema()` covering quoted, unquoted, and `SESSION` forms.
   7.2 Add `#[test] fn rejects_set_search_path_multi_schema()` asserting `Reject` with the expected message.
   7.3 Add `#[test] fn handles_search_path_reset_and_default()` covering `RESET search_path` and `SET search_path = DEFAULT`.
   7.4 Add `#[test] fn classifies_show_search_path_dynamically()` asserting the new `ShowSearchPath` variant.

8. **Add JDBC compatibility probe**
   8.1 Update `tests/jdbc/PgJdbcCompatibilitySuite.java` so the existing `set-search-path` probe (currently `EXPLORATORY`, currently sends `SET search_path TO pg_demo, pg_catalog`) is split into:
       - `set-search-path-single` (`MUST_PASS`) — sends `SET search_path = "PG_DEMO"`, then `SELECT current_schema()` and asserts the returned schema name.
       - `set-search-path-multi` (`MUST_PASS`) — sends `SET search_path TO pg_demo, pg_catalog` and asserts the gateway returns a SQL error referencing single-schema-only.
       - `reset-search-path` (`MUST_PASS`) — sends `RESET search_path` and expects success.
       - `show-search-path` (`MUST_PASS`) — sends `SHOW search_path` after a successful single-schema set and asserts the value matches.

9. **Update CHANGELOG / docs**
   9.1 Document the new gateway-managed compatibility behavior in `README.md` (or the operator-facing docs section that lists session-command compatibility).

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A — classifier | 1, 2, 3 |
| Group B — handler | 4, 5, 6 |
| Group C — verification | 7, 8 |
| Group D — docs | 9 |

Sequential dependencies:
- Group A → Group B (handler matches on the new variants from A).
- Group A + Group B → Group C (integration tests need both classifier and handler).
- Group C → Group D (docs reflect verified behavior).

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| Static value | `local_show()` in `src/policy.rs` — the hardcoded `"search_path" => "public"` arm | Replaced by the new dynamic `ShowSearchPath` plan; the static value moves to the handler default. |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| Single-schema search_path opens the Exasol schema | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `set-search-path-single` probe |
| Multi-schema search_path is rejected with a compatibility error | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `set-search-path-multi` probe |
| search_path reset is a no-op | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `reset-search-path` probe |
| SHOW search_path reflects the active schema | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `show-search-path` probe |
| search_path open failure surfaces as a PostgreSQL-compatible error | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` | new `set-search-path-missing-schema` probe — sends `SET search_path = "DOES_NOT_EXIST"` and asserts a SQL error |
| search_path session command maps to Exasol OPEN SCHEMA (matrix entry) | Unit | `src/policy.rs` `#[cfg(test)] mod tests` | `classifies_set_search_path_single_schema` |
| SHOW search_path reads gateway-managed session state (matrix entry) | Unit | `src/policy.rs` `#[cfg(test)] mod tests` | `classifies_show_search_path_dynamically` |
| search_path session state is owned by the gateway | Integration | `tests/jdbc/PgJdbcCompatibilitySuite.java` | `set-search-path-single` + `show-search-path` probes verify state ownership end-to-end |

The classifier scenarios are pure-computation policy decisions with no I/O, so unit tests in `src/policy.rs` satisfy the integration-by-default rule per the unit-test exception. End-to-end behavior (handler + Exasol) is covered by the JDBC compatibility suite which exercises the full wire path.

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| protocol/read-write-statement-path | `psql "host=localhost port=5433 user=sys dbname=exasol" -c 'SET search_path = "PG_DEMO"; SELECT current_schema();'` | Two command rows: `SET` followed by a single-row result whose `current_schema` value is `PG_DEMO`. |
| protocol/read-write-statement-path | `psql "host=localhost port=5433 user=sys dbname=exasol" -c 'SET search_path = pg_demo, pg_catalog;'` | A SQL error stating that only single-schema search paths are supported; session remains usable for the next statement. |
| protocol/read-write-statement-path | `psql "host=localhost port=5433 user=sys dbname=exasol" -c 'RESET search_path;'` | A `RESET` (or `SET`) command tag with no error. |
| sql/postgres-to-exasol-compatibility | `psql "host=localhost port=5433 user=sys dbname=exasol" -c 'SET search_path = "PG_DEMO"; SHOW search_path;'` | After `SET`, `SHOW search_path` returns one row with value `PG_DEMO`. |
| operations/gateway-owned-translation | Connect with DBeaver to the gateway and open the schema editor for an Exasol schema, then run a query that depends on schema context. | DBeaver opens the editor without raising the `Cannot invoke "java.lang.CharSequence.toString()" because "s" is null` NPE; subsequent unqualified queries resolve through the opened schema. |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Format | `cargo fmt` | No changes |
| Build | `cargo build --release` | Exit 0 |
| Test | `cargo test` | 0 failures |
| Integration | `python3 scripts/exasol_exec.py --dsn EXASOL_HOST:8563 --user sys --password 'EXASOL_PASSWORD' --sql "SELECT 1"` | Exit 0, returns `1` |
| JDBC compat suite | `cd tests/jdbc && javac PgJdbcCompatibilitySuite.java && java PgJdbcCompatibilitySuite ...` (per existing project invocation) | All `MUST_PASS` probes including the new `set-search-path-*` and `show-search-path` probes pass |
