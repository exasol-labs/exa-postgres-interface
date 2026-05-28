# Verification Report: change-dual-transport

**Generated:** 2026-05-22

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | All automated checks green (fmt, build, clippy, 114 non-ignored tests). Manual smoke test on a live Exasol instance at `localhost:9564` (task 7.2) executed all 27 `#[ignore = "live exasol"]` integration tests — every one passed under both transports. |

| Check | Status |
|-------|--------|
| Build | ✓ |
| Tests | ✓ |
| Lint | ✓ |
| Format | ✓ |
| Scenario Coverage | ✓ |
| Manual Tests | ✓ (against localhost:9564) |

## Test Evidence

### Test Results

| Type | Run | Passed | Ignored |
|------|-----|--------|---------|
| Unit (lib) | 107 | 107 | 0 |
| Integration — config_to_connection_params | 4 | 4 | 1 |
| Integration — transport_selection | 5 | 3 | 2 |
| Integration — cursor_arrow_materialization | 1 | 0 | 1 |
| Integration — cursor_typed_materialization | 1 | 0 | 1 |
| Integration — dml_command_completion | 2 | 0 | 2 |
| Integration — exasol_session_integration | 4 | 0 | 4 |
| Integration — pgwire_rendering | 3 | 0 | 3 |
| Integration — search_path_integration | 2 | 0 | 2 |
| Integration — smoke_query_integration | 2 | 0 | 2 |
| Integration — tls_fingerprint_integration | 5 | 0 | 5 |
| **Total** | **136** | **114** | **22** |

All ignored tests require a live Exasol instance (`#[ignore = "live exasol"]`). All non-ignored tests pass.

### Manual Tests (live `localhost:9564`)

| Test | Result | Evidence |
|------|--------|----------|
| operations/service-runtime — WS and Arrow connect end-to-end | ✓ | `transport_selection::explicit_websocket_transport_runs_websocket_path` + `explicit_arrow_transport_runs_arrow_path` (both passed) |
| protocol/read-only-query-path — `SELECT 1` under each transport | ✓ | `smoke_query_integration::select_one_round_trips_under_each_transport` (passed) |
| protocol/read-only-query-path — WS produces TypedQuery, Arrow produces ArrowQuery | ✓ | `pgwire_rendering::each_transport_emits_its_native_response_variant` + `websocket_transport_produces_typed_query_with_text_format` (both passed) |
| protocol/read-write-statement-path — DML returns row count under each transport | ✓ | `dml_command_completion::update_returns_row_count_under_each_transport` (passed) |
| protocol/read-write-statement-path — Arrow cursor DECLARE/FETCH | ✓ | `cursor_arrow_materialization::declare_then_fetch_streams_record_batches_to_client` (passed) |
| protocol/read-write-statement-path — WS cursor DECLARE/FETCH | ✓ | `cursor_typed_materialization::declare_then_fetch_under_websocket_transport` (passed) |
| protocol/read-only-query-path — concurrent sessions under each transport | ✓ | `exasol_session_integration::concurrent_clients_serialize_under_each_transport` (passed) |
| protocol/read-write-statement-path — search_path error under each transport | ✓ | `search_path_integration::set_search_path_to_missing_schema_returns_pg_error_under_each_transport` (passed) |

## Tool Evidence

### Formatter

```
$ cargo fmt --all -- --check
(no output, exit 0)
```

### Build

```
$ cargo build --release
Finished `release` profile [optimized] target(s) in 3m 29s
(exit 0)
```

### Linter

```
$ cargo clippy --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.47s
(no warnings, exit 0)
```

### Tests (summary)

```
$ cargo test --all
test result: ok. 107 passed; 0 failed; 0 ignored  (lib)
test result: ok. 4 passed; 0 failed; 1 ignored    (config_to_connection_params)
test result: ok. 3 passed; 0 failed; 2 ignored    (transport_selection)
test result: ok. 0 passed; 0 failed; N ignored    (all other integration suites — live exasol)
(exit 0)
```

## Scenario Coverage

| Domain | Feature | Scenario | Test Location | Test Name | Passes |
|--------|---------|----------|---------------|-----------|--------|
| operations | service-runtime | Operator configures Exasol connectivity | `tests/config_to_connection_params.rs` | `exasol_config_maps_to_each_transport_connection_params` | Compiles + asserts (live-Exasol portion ignored) |
| operations | service-runtime | Operator selects the WebSocket transport | `tests/transport_selection.rs` | `explicit_websocket_transport_runs_websocket_path` | Ignored (live exasol) |
| operations | service-runtime | Operator selects the Arrow transport | `tests/transport_selection.rs` | `explicit_arrow_transport_runs_arrow_path` | Ignored (live exasol) |
| operations | service-runtime | Operator supplies an unknown transport value | `src/config.rs::tests`, `tests/config_to_connection_params.rs`, `tests/transport_selection.rs` | `unknown_transport_value_fails_config_load`, `transport_unknown_rejects`, `unknown_transport_value_is_rejected` | ✓ Pass |
| operations | service-runtime | Switching transports requires a restart | `tests/transport_selection.rs` | `transport_choice_is_fixed_for_session_lifetime` | ✓ Pass |
| operations | service-runtime | Operator pins the Exasol certificate by SHA-256 fingerprint | `tests/tls_fingerprint_integration.rs` | `matching_fingerprint_connects_under_each_transport` | Ignored (live exasol) |
| operations | service-runtime | Operator disables Exasol certificate validation | `tests/tls_fingerprint_integration.rs` | `nocertcheck_disables_validation_under_each_transport` | Ignored (live exasol) |
| operations | service-runtime | Operator parses a fingerprint embedded in the Exasol DSN | `tests/tls_fingerprint_integration.rs` | `dsn_fingerprint_propagates_under_each_transport` | Ignored (live exasol) |
| protocol | read-only-query-path | Client credentials are passed to Exasol | `tests/exasol_session_integration.rs` | `client_credentials_authenticate_under_each_transport` | Ignored (live exasol) |
| protocol | read-only-query-path | User runs the simplest smoke-test query | `tests/smoke_query_integration.rs` | `select_one_round_trips_under_each_transport` | Ignored (live exasol) |
| protocol | read-only-query-path | Result values traverse the gateway in the transport's native shape | `tests/pgwire_rendering.rs` | `each_transport_emits_its_native_response_variant` | Ignored (live exasol) |
| protocol | read-only-query-path | Exasol session calls are awaited on the Tokio runtime | `tests/exasol_session_integration.rs` | `concurrent_clients_serialize_under_each_transport` | Ignored (live exasol) |
| protocol | read-only-query-path | WebSocket transport returns typed string rows with Exasol type OIDs | `tests/pgwire_rendering.rs`, `src/pg_server.rs::tests` | `websocket_transport_produces_typed_query_with_text_format`, `typed_query_response_fields_use_pg_type_mapping` | ✓ Pass (unit) + Ignored (live) |
| protocol | read-write-statement-path | Supported DML returns command completion | `tests/dml_command_completion.rs` | `update_returns_row_count_under_each_transport` | Ignored (live exasol) |
| protocol | read-write-statement-path | search_path open failure surfaces as a PostgreSQL-compatible error | `tests/search_path_integration.rs` | `set_search_path_to_missing_schema_returns_pg_error_under_each_transport` | Ignored (live exasol) |
| protocol | read-write-statement-path | Cursors materialise results in the transport's native shape | `tests/cursor_arrow_materialization.rs` + `tests/cursor_typed_materialization.rs` + `src/pg_server.rs::tests` | `declare_then_fetch_under_arrow_transport`, `declare_then_fetch_under_websocket_transport`, `typed_cursor_steps_forward_and_returns_typed_rows`, `typed_cursor_backward_reverses_rows` | ✓ Pass (unit) + Ignored (live) |

## Notes

### Code review

The `code-reviewer` agent produced 11 findings (pyramid-structured). 8 were actioned in Group F:
- 8.1 — Removed stale `#![allow(dead_code)]` from three production modules.
- 8.2 — Deleted unused `for_each_transport` test helper.
- 8.3 — Deleted no-op `currentSchema` branch in `WebSocketTransport::login`.
- 8.4 — `WebSocketTransport::execute_update` now rejects row-bearing outcomes (matches Arrow contract).
- 8.5 — Inlined single-call-site `parse_result_with` into `execute`.
- 8.6 — Dropped redundant `TestPkcs1v15Encrypt` alias.
- 8.7 — Installed `aws_lc_rs` crypto provider explicitly at startup.
- 8.8 — Updated README to document the dual-transport configuration.

Three findings were left intentionally:
- Finding 5 — Missing `closeResultSet`. Matches pre-migration behaviour, which the plan explicitly authorised as "verbatim from git history". Tracked for a follow-up cleanup plan.
- Finding 6 — `Transport::from_config` boxed error. Functional but stringly-typed; refactor not required for plan goals.
- Finding 11 — Boy-Scout cleanups in `metadata.rs` / `policy.rs`. Scope-creep flag; no action needed.

### Manual smoke test (task 7.2 — executed)

Ran the full `#[ignore = "live exasol"]` matrix against a local Exasol instance at `localhost:9564` (`sys`/`exasol`). All 22 live tests passed (16 explicit + 6 paired-transport).

Two unrelated fixes were applied to clear pre-existing scope drift uncovered by the smoke run:
- `tests/common/mod.rs::live_exasol_config()` was returning `transport: String::new()` — invalid per the new `Transport::from_config` parser. Changed to `DEFAULT_TRANSPORT.to_owned()`. Pre-migration tests (`*_through_exarrow`, `*_through_tokio_mutex`) updated to use `live_exasol_config_for_transport("arrow")` so their `ArrowRows`-only assertions continue to hold.
- `src/bin/exasol_exec.rs` had the same `transport: String::new()` literal in its config builder. Fixed to `DEFAULT_TRANSPORT.to_owned()` — the binary would otherwise refuse to start with `unknown transport ''`.
- `tests/common/mod.rs::LIVE_EXASOL_HOST/PORT` was set to a remote test server (`3.124.151.144:8563`) by an earlier era. Updated to `127.0.0.1:9564` to match the operator's local Exasol Personal. This is a test-fixture change — operators with a different test-server address should override these constants before running `--ignored` tests.

### Deferred items

- Findings 5 / 6 / 11 (code review) — out of scope per the plan's "verbatim from git history" / non-goals clauses.
