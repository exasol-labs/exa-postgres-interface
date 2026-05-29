# Verification Report: change-exarrow-transport

**Generated:** 2026-05-18

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | All automated checks pass, including 10 live integration tests against a real Exasol instance at `3.124.151.144:8563`. |

| Check | Status |
|-------|--------|
| Build (`cargo build --release`) | ✓ |
| Unit tests (`cargo test --all`) | ✓ 92 passed |
| Live integration tests (`cargo test --all -- --ignored`) | ✓ 10 passed |
| Lint (`cargo clippy --all-targets --all-features -- -D warnings`) | ✓ 0 warnings |
| Format (`cargo fmt --all -- --check`) | ✓ no changes |

## Test Evidence

### Test Results

| Type | Run | Passed | Ignored |
|------|-----|--------|---------|
| Unit (main binary) | 85 | 85 | 0 |
| Unit (`exasol_exec` binary) | 7 | 7 | 0 |
| Live integration tests | 10 | 10 | 0 (when run with `--ignored`) |

### Live test breakdown

| Test | Result |
|------|--------|
| `tests/exasol_session_integration.rs::client_credentials_authenticate_through_exarrow` | ✓ |
| `tests/exasol_session_integration.rs::concurrent_clients_serialize_through_tokio_mutex` | ✓ |
| `tests/smoke_query_integration.rs::select_one_round_trips_arrow_through_pgwire` | ✓ |
| `tests/pgwire_arrow_rendering.rs::record_batches_render_into_pgwire_data_rows` | ✓ |
| `tests/dml_command_completion.rs::update_returns_exasol_row_count_through_arrow_outcome` | ✓ |
| `tests/search_path_integration.rs::set_search_path_to_missing_schema_returns_pg_error` | ✓ |
| `tests/cursor_arrow_materialization.rs::declare_then_fetch_streams_record_batches_to_client` | ✓ |
| `tests/config_to_connection_params.rs::exasol_config_maps_to_exarrow_connection_params` | ✓ |
| `tests/tls_fingerprint_integration.rs::matching_fingerprint_connects_mismatched_fingerprint_rejected` | ✓ |
| `tests/tls_fingerprint_integration.rs::nocertcheck_disables_validation_with_warning_log` | ✓ |

### Live test environment

- Exasol instance: `3.124.151.144:8563`, user `sys`, self-signed TLS
- TLS fingerprint captured: `A996DAAA5D6AB45075CDC12E8EE219DEE571F8A60FA0E4796C003AC939759393` (used by `matching_fingerprint_connects_mismatched_fingerprint_rejected`)
- `PG_CATALOG` and `INFORMATION_SCHEMA` compatibility schemas installed via `scripts/exasol_exec.py --file sql/postgres_catalog_compatibility.sql`
- Run command: `cargo test --all -- --ignored --test-threads=1`

## Tool Evidence

### Linter

```
cargo clippy --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 00s
(exit 0 — no warnings)
```

### Formatter

```
cargo fmt --all -- --check
(exit 0 — no changes)
```

### Build

```
cargo build --release
Finished `release` profile [optimized] target(s) in 3m 52s
(exit 0)
```

## Scenario Coverage

| Domain | Feature | Scenario | Test Location | Test Name | Status |
|--------|---------|----------|---------------|-----------|--------|
| protocol | read-only-query-path | Client credentials are passed to Exasol | `tests/exasol_session_integration.rs` | `client_credentials_authenticate_through_exarrow` | **Pass (live)** |
| protocol | read-only-query-path | User runs the simplest smoke-test query | `tests/smoke_query_integration.rs` | `select_one_round_trips_arrow_through_pgwire` | **Pass (live)** |
| protocol | read-only-query-path | Result values traverse the gateway as Apache Arrow record batches | `tests/pgwire_arrow_rendering.rs` | `record_batches_render_into_pgwire_data_rows` | **Pass (live)** |
| protocol | read-only-query-path | Exasol session calls are awaited on the Tokio runtime | `tests/exasol_session_integration.rs` | `concurrent_clients_serialize_through_tokio_mutex` | **Pass (live)** |
| protocol | read-write-statement-path | Supported DML returns command completion | `tests/dml_command_completion.rs` | `update_returns_exasol_row_count_through_arrow_outcome` | **Pass (live)** |
| protocol | read-write-statement-path | search_path open failure surfaces as PostgreSQL-compatible error | `tests/search_path_integration.rs` | `set_search_path_to_missing_schema_returns_pg_error` | **Pass (live)** |
| protocol | read-write-statement-path | Cursors materialise Arrow record batches from Exasol | `tests/cursor_arrow_materialization.rs` | `declare_then_fetch_streams_record_batches_to_client` | **Pass (live)** |
| operations | service-runtime | Operator configures Exasol connectivity | `tests/config_to_connection_params.rs` | `exasol_config_maps_to_exarrow_connection_params` | **Pass (live)** |
| operations | service-runtime | Operator pins the Exasol certificate by SHA-256 fingerprint | `tests/tls_fingerprint_integration.rs` | `matching_fingerprint_connects_mismatched_fingerprint_rejected` | **Pass (live)** |
| operations | service-runtime | Operator disables Exasol certificate validation | `tests/tls_fingerprint_integration.rs` | `nocertcheck_disables_validation_with_warning_log` | **Pass (live)** |
| operations | service-runtime | Operator parses a fingerprint embedded in the Exasol DSN | `src/exasol.rs::tests` | `dsn_fingerprint_propagates_to_connection_params` | Pass |

## Notes

- **Library surface added**: to support integration tests, the crate now exposes a `src/lib.rs` re-exporting `config`, `exasol`, and `pg_server`. The binary continues to build via `src/main.rs`.
- **Test fixtures isolate cleanly**: each test that creates database state uses a unique schema name and drops it at the end. The DML test uses `exa_pg_test_dml`; the cursor test uses an in-line `VALUES` form.
- **Live fingerprint pinned in test constants**: `tests/common/mod.rs::LIVE_FINGERPRINT_HEX` carries the captured SHA-256 fingerprint. If the Exasol server certificate rotates, the matching-fingerprint test will start failing and the constant must be refreshed.
- **`Connection::from_params` vs plan prose**: `exarrow-rs 0.12.2` exposes `Connection::from_params(params)` rather than `Connection::connect(params)`. Used `from_params` directly.
- **`query_response_arrow` streams per-batch, not per-row**: each batch materializes a bounded `Vec<PgWireResult<DataRow>>` before the next batch is processed. Truly per-row laziness would require self-referential iterators due to `ArrayFormatter<'_>` lifetime constraints; per-batch is the right trade-off given the typical batch size (≤8192 rows).
- **Task 6.1 (spec doc update)**: deferred to `speq:record` as stated in the plan.
