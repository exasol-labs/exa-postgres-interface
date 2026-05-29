# Plan: Change Exarrow Transport

## Summary

Replace the hand-rolled synchronous WebSocket transport in `src/exasol.rs` with the `exarrow-rs` async driver, propagate Apache Arrow `RecordBatch` results through `pg_server.rs`, and make `ExasolSession` fully async while preserving the existing TLS fingerprint / `NOCERTCHECK` configuration surface.

## Design

### Context

The gateway currently opens Exasol sessions through a hand-rolled `tungstenite` WebSocket loop. The transport does its own RSA-encrypted login (`rsa` + `sha2`), its own TLS via `native-tls`, and a manual JSON request/response loop with Ping/Pong handling. Results land in a `ResultSet { columns: Vec<ExasolColumn>, rows: Vec<Vec<Option<String>>> }` shape that pre-stringifies every Exasol value before the PostgreSQL wire layer sees it. The Tokio-based pgwire handler then has to wrap every Exasol call in `task::spawn_blocking` and protect the session with a `std::sync::Mutex`.

`exarrow-rs` (v0.12, MIT, exasol-labs) is now the upstream Rust driver. Its public API (`Driver`, `Database`, `Connection`, `ResultSet`, `RecordBatch`) is async on Tokio, transports either over native TCP or WebSocket, uses `rustls` + `aws-lc-rs` instead of `rsa`/`native-tls`, and accepts `validate_server_certificate` plus `certificate_fingerprint` directly on its `ConnectionParams`. Moving to it removes ~400 lines of hand-rolled protocol code, eliminates `block_in_place`-style juggling, and lets the gateway carry Arrow `RecordBatch` values as its native result shape.

- **Goals**
    - Replace `src/exasol.rs` transport with `exarrow-rs` while keeping the current configuration surface.
    - Propagate Apache Arrow `RecordBatch` results all the way through `pg_server.rs` instead of `Vec<Vec<Option<String>>>`.
    - Preserve TLS fingerprint pinning and the `NOCERTCHECK` escape hatch.
    - Make `ExasolSession` fully async; remove `spawn_blocking` and `std::sync::Mutex` around Exasol calls.
- **Non-Goals**
    - Adopting prepared statements, transactions, bulk import/export, or any other `exarrow-rs` capability beyond `connect` / `execute` / `query` / `set_schema` / `close`.
    - Changing the existing `ExasolConfig` TOML keys or PostgreSQL wire-protocol contract.
    - Changing the PostgreSQL-to-Exasol type-mapping policy beyond what is required to render Arrow columns into `pgwire` field info.
    - Replacing the gateway-managed cursor design (cursors continue to materialise their result up front; only the storage shape changes from `Vec<Vec<Option<String>>>` to `Vec<RecordBatch>`).

### Decision

#### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                       pg_server.rs                           │
│ ┌──────────────────────┐  ┌──────────────────────────────┐   │
│ │ ExasolPgWireHandler  │  │ SessionState                 │   │
│ │ (async)              │  │  exasol: tokio::sync::Mutex<ExasolSession>
│ │                      │  │  cursors: tokio::sync::Mutex<..>             
│ │  encode_record_batches_for_pgwire()
│ └──────────┬───────────┘  └────────────┬─────────────────┘   │
│            │ .await                    │                    │
│            ▼                           ▼                    │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ src/exasol.rs                                            │ │
│ │   pub struct ExasolSession { inner: exarrow_rs::Connection }
│ │   pub async fn connect(...)                              │ │
│ │   pub async fn initialize(...)                           │ │
│ │   pub async fn execute(...) -> ExasolOutcome             │ │
│ │   pub async fn close(...)                                │ │
│ └────────────────────────────┬─────────────────────────────┘ │
└──────────────────────────────│───────────────────────────────┘
                               ▼
                    ┌────────────────────┐
                    │   exarrow-rs       │
                    │ (async, rustls,    │
                    │  Arrow RecordBatch)│
                    └────────────────────┘
```

The gateway keeps the existing module boundaries: `exasol.rs` owns the driver shape, `pg_server.rs` owns the wire-protocol mapping, and `bootstrap.rs` keeps its synchronous CLI ergonomics via the existing top-level Tokio runtime (`main.rs` already builds a multi-thread runtime and runs `run()` inside `block_on`).

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Thin facade over `exarrow_rs::Connection` | `src/exasol.rs` | Preserves the call sites (`connect` / `initialize` / `execute`) and contains config-to-`ConnectionParams` translation, fingerprint plumbing, and DSN parsing. |
| Async-only session API | `ExasolSession::{connect, initialize, execute, close}` | Lets `pg_server.rs` `.await` Exasol directly and removes the cost of `spawn_blocking` per query. |
| `tokio::sync::Mutex` for shared session | `SessionState::exasol` | Required because the session is held across `.await` points; `std::sync::Mutex` cannot be held across `await`. |
| Arrow-batches-in, pgwire-out renderer | `pg_server.rs::encode_record_batches_for_pgwire` | One place to map Arrow `DataType` → pgwire `Type` and Arrow values → PostgreSQL text-format bytes. |
| Outcome enum (`ExasolOutcome::Rows(Vec<RecordBatch>)` / `RowCount(i64)`) | `src/exasol.rs` | Keeps the row-count vs. result-set distinction the gateway already uses, but the row-set arm now carries Arrow batches. |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| Adopt `exarrow-rs` and delete the hand-rolled WebSocket/JSON/RSA login code | (a) Keep the current transport and merely retype results to `RecordBatch`. (b) Wrap `exarrow-rs` behind an adapter that re-exports the old string-row shape. | (a) loses the maintenance win and keeps `rsa` + `native-tls` in the dependency graph. (b) keeps a redundant intermediate shape and forfeits the Arrow throughput improvement. Replacing the transport outright is what the user intent calls for. |
| Push `RecordBatch` into `pg_server.rs` instead of stringifying rows in `exasol.rs` | A compatibility shim that converts every Arrow column to `Option<String>` before leaving `exasol.rs`. | The user explicitly asked for full Arrow propagation. The shim would force a value-by-value round trip through `String` for every cell, which is exactly the cost the migration is trying to remove. |
| Use `exarrow-rs`' built-in `certificate_fingerprint` and `validate_server_certificate` `ConnectionParams` | Build a custom `rustls` `ServerCertVerifier` and inject it into `exarrow-rs`. | `exarrow-rs::ConnectionParams` already exposes both fields verbatim (`connection/params.rs::ConnectionBuilder`), and the transport applies them inside its own TLS setup. A custom verifier would re-implement what the driver already does and would lock us into an internal `exarrow-rs` API surface. |
| Make `ExasolSession` fully async and replace `Mutex<ExasolSession>` with `tokio::sync::Mutex<ExasolSession>` | Keep the sync API and continue using `task::spawn_blocking`. | The user intent is explicit: no `block_in_place`, no `spawn_blocking`. `tokio::sync::Mutex` is required because the guard must survive `.await` points on the driver. |
| Run `bootstrap.rs` interactive flow via the existing Tokio runtime's `block_on` | Add `#[tokio::main]` to `main.rs` or split bootstrap into a separate runtime. | `main.rs` already constructs the runtime, so `bootstrap.rs` can take a `tokio::runtime::Handle` (or be called from inside `run()` after `Runtime::block_on`). This keeps a single runtime for the whole process. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| protocol/read-only-query-path | CHANGED | `specs/_plans/change-exarrow-transport/protocol/read-only-query-path/spec.md` |
| protocol/read-write-statement-path | CHANGED | `specs/_plans/change-exarrow-transport/protocol/read-write-statement-path/spec.md` |
| operations/service-runtime | CHANGED | `specs/_plans/change-exarrow-transport/operations/service-runtime/spec.md` |

## Dependencies

Cargo additions:

| Crate | Reason |
|-------|--------|
| `exarrow-rs = "0.12"` (features: `["native"]` by default; revisit `["websocket"]` only if native TCP turns out to be unsupported against the target Exasol deployment) | Async Exasol driver. |
| `arrow = "57.1"` (matching `exarrow-rs`' transitive version) | `RecordBatch`, `Schema`, `Field`, `DataType`, and the typed array downcasts the gateway needs to render Arrow values into PostgreSQL text format. |

Cargo removals:

| Crate | Reason |
|-------|--------|
| `tungstenite` | Replaced by `exarrow-rs` transport. |
| `native-tls` | Replaced by `rustls` (already in the build via `tokio-rustls` and now via `exarrow-rs`). |
| `rsa` | Exasol login is handled inside `exarrow-rs`. |
| `sha2` | Fingerprint hashing now lives inside `exarrow-rs`. |
| `base64` | Only used by the old RSA password encryption path. |

`tokio` `sync` feature stays enabled (`tokio::sync::Mutex` is required). `tokio-rustls` and `rustls-pemfile` stay because they back the PostgreSQL-side TLS listener, which is a separate concern.

## Migration

| Current | New |
|---------|-----|
| `ExasolSession { ws: WebSocket<ExaStream> }` | `ExasolSession { inner: exarrow_rs::Connection }` |
| `pub fn connect(config, user, pw) -> Result<Self, ExasolError>` | `pub async fn connect(config, user, pw) -> Result<Self, ExasolError>` |
| `pub fn execute(&mut self, sql) -> Result<ExasolResult, ExasolError>` | `pub async fn execute(&mut self, sql) -> Result<ExasolOutcome, ExasolError>` |
| `ExasolResult::ResultSet { columns: Vec<ExasolColumn>, rows: Vec<Vec<Option<String>>> }` | `ExasolOutcome::Rows(Vec<arrow::array::RecordBatch>)` |
| `ExasolResult::RowCount(usize)` | `ExasolOutcome::RowCount(i64)` (matching `exarrow_rs::ResultSet::row_count`) |
| `SessionState { exasol: Mutex<ExasolSession>, ... }` (std mutex) | `SessionState { exasol: tokio::sync::Mutex<ExasolSession>, cursors: tokio::sync::Mutex<..>, current_schema: tokio::sync::Mutex<..>, extended_results: tokio::sync::Mutex<..> }` |
| `task::spawn_blocking(\|\| session.execute(&sql))` in `pg_server.rs` | `session.lock().await.execute(&sql).await` |
| `GatewayCursor { columns: Vec<GatewayColumn>, rows: Vec<Vec<Option<String>>>, position, scroll, hold }` | `GatewayCursor { schema: arrow::datatypes::SchemaRef, batches: Vec<RecordBatch>, position: isize, scroll, hold }` (cursor stepping operates on flat row indices into the concatenated batches) |
| `map_exasol_result(...) -> GatewayResponse::TypedQuery { columns, rows: Vec<Vec<Option<String>>>, ... }` | `map_exasol_result(...) -> GatewayResponse::ArrowQuery { schema: SchemaRef, batches: Vec<RecordBatch>, command_tag }` (the `Empty` / `Execution` / `TransactionStart` / `TransactionEnd` / `Error` arms stay untouched) |
| `query_response_typed(columns, rows: Vec<Vec<Option<String>>>, command_tag)` | `query_response_arrow(schema: SchemaRef, batches: Vec<RecordBatch>, command_tag)` — encodes each row by downcasting Arrow arrays and writing through `DataRowEncoder` |
| `fetch_query_rows -> Vec<Vec<Option<String>>>` (consumed by metadata helpers in `pg_server.rs`) | `fetch_query_rows -> Vec<Vec<Option<String>>>` is kept as a thin Arrow-to-text helper because the metadata layer post-processes rows as strings; the helper now reads from `RecordBatch` columns via a single `arrow_batch_to_text_rows` utility |
| `bootstrap.rs::ExasolSession::connect(...)` (sync) | Bootstrap calls `runtime.block_on(ExasolSession::connect(...))` using a `tokio::runtime::Handle` passed in from `main.rs` |

## Implementation Tasks

- [ ] 1.1 Update `Cargo.toml`: add `exarrow-rs = "0.12"` (with `default-features = false` + `features = ["native"]`) and `arrow = "57.1"`; remove `tungstenite`, `native-tls`, `rsa`, `sha2`, and `base64`; run `cargo update -p exarrow-rs` and confirm the lockfile resolves.
- [ ] 1.2 Verify `exarrow-rs` resolves `arrow` to a single shared version with the gateway and that the `aws-lc-rs` rustls backend it pulls in does not conflict with `pgwire`'s `server-api-aws-lc-rs` feature.
- [ ] 2.1 Rewrite `src/exasol.rs`: keep `ExasolError`, replace `ExasolColumn`/`ExasolResult` with `ExasolOutcome { Rows(Vec<RecordBatch>), RowCount(i64) }`, and define `ExasolSession { inner: exarrow_rs::Connection }`.
- [ ] 2.2 Implement `ExasolSession::connect(config, username, password)` async: build `exarrow_rs::ConnectionParams` via its `ConnectionBuilder`, apply `host` / `port` (from existing DSN parsing), `username`, `password`, `schema` (when `config.schema` non-empty), `use_tls = config.encryption`, `validate_server_certificate = config.validate_certificate`, and `certificate_fingerprint` resolved from `ExasolConfig` or the DSN suffix. Map driver errors into the existing `ExasolError` variants. [expert]
- [ ] 2.3 Reuse the existing `Endpoint::parse` precedence so `ExasolConfig.certificate_fingerprint` overrides a fingerprint embedded in the DSN, and so `validate_certificate = false` keeps producing the `NOCERTCHECK`-style behavior by setting `validate_server_certificate = false` on `exarrow-rs` `ConnectionParams` and leaving the fingerprint empty. [expert]
- [ ] 2.4 Implement `ExasolSession::initialize(&mut self, &[String], &str)` async; rewrite the `{script}` placeholder substitution and call `Connection::execute_update` for each statement.
- [ ] 2.5 Implement `ExasolSession::execute(&mut self, &str)` async: call `Connection::execute(...)`, then branch on `ResultSet::row_count()` vs `ResultSet::fetch_all()` and return the matching `ExasolOutcome` arm.
- [ ] 2.6 Implement `Drop` / explicit `close`: provide an `async fn close(self)` that calls `Connection::close().await`; keep a best-effort `Drop` that logs but cannot `.await`.
- [ ] 2.7 Rewrite the `src/exasol.rs` unit tests: keep `appends_nocertcheck_policy_from_config` and `preserves_dsn_fingerprint` against the new `Endpoint`-to-`ConnectionParams` adapter; delete the `Message::Pong`/`Message::Text` tests because they live inside `exarrow-rs` now.
- [ ] 3.1 In `src/pg_server.rs`, change `SessionState.exasol` from `std::sync::Mutex<ExasolSession>` to `tokio::sync::Mutex<ExasolSession>`; do the same for `extended_results`, `cursors`, and `current_schema` to remove poisoning handling.
- [ ] 3.2 Remove every `task::spawn_blocking` wrapping an Exasol call; replace each call site with `session.lock().await.execute(&sql).await` (and the analogous `set_schema` for `SetSearchPath`). Update the error mapping closures accordingly. [expert]
- [ ] 3.3 Replace `ExasolResult` consumers in `pg_server.rs` with `ExasolOutcome` consumers: `execute_exasol_sql`, `execute_client_sql`, `execute_exasol_query`, `fetch_query_rows`, `declare_cursor`, `execute` (StatementPlan::Execute), and `map_exasol_result`.
- [ ] 3.4 Define a new `GatewayResponse::ArrowQuery { schema: arrow::datatypes::SchemaRef, batches: Vec<RecordBatch>, command_tag: Option<String> }` variant (replacing the existing `TypedQuery` variant) and update every constructor + `TryInto<Response>` arm. [expert]
- [ ] 3.5 Implement an Arrow-to-pgwire renderer: `query_response_arrow(schema, batches, command_tag) -> PgWireResult<QueryResponse>` that builds `FieldInfo` per Arrow `Field` (mapping `DataType` → pgwire `Type`) and streams rows by downcasting each `ArrayRef` and writing through `DataRowEncoder` in text format. Cover the Arrow types `exarrow-rs` returns for Exasol's `BOOLEAN`, `DECIMAL`, `DOUBLE`, `DATE`, `TIMESTAMP`, `TIMESTAMP WITH LOCAL TIME ZONE`, `VARCHAR/CHAR/HASHTYPE`. [expert]
- [ ] 3.6 Rebuild `GatewayCursor`: store `schema: SchemaRef` plus `batches: Vec<RecordBatch>` and rewrite `forward`/`backward`/`absolute`/`relative`/`apply` to operate on a flat row index that walks across batches. Render fetched slices through the same Arrow-to-pgwire renderer. [expert]
- [ ] 3.7 Rewrite `fetch_query_rows` as a private `arrow_batches_to_text_rows(batches: Vec<RecordBatch>) -> Vec<Vec<Option<String>>>` helper kept for the metadata code that still treats column values as strings (`MetadataPlan::PgAttribute`, etc.); the wire path SHALL go through the new renderer instead.
- [ ] 3.8 Update the `map_exasol_columns` / `pg_type_for_exasol_data_type` paths to consume Arrow `DataType` + `Field` metadata instead of the Exasol `dataType` JSON, keeping the existing OID mapping policy (`Type::INT4`/`NUMERIC`/`FLOAT8`/`DATE`/`TIMESTAMP`/`TIMESTAMPTZ`/`VARCHAR`).
- [ ] 3.9 Refresh the in-file `#[cfg(test)] mod tests` in `pg_server.rs`: rebuild fixtures for `GatewayResponse::ArrowQuery` (small `RecordBatch` literals) so `map_exasol_result_*` and `query_response_arrow` round-trip tests stay meaningful.
- [ ] 4.1 In `src/bootstrap.rs`, take a `tokio::runtime::Handle` (passed from `main.rs::run` after the runtime is built) and call `handle.block_on(ExasolSession::connect(...))` and `handle.block_on(session.execute(...))` from the synchronous prompt flow. Keep terminal I/O synchronous.
- [ ] 4.2 Replace `ExasolResult` usage in `bootstrap.rs::first_count` with `ExasolOutcome::Rows(batches)` → read first row, first column as `i64` (Exasol returns `COUNT(*)` as a decimal); convert through Arrow's `as_primitive::<Int64Type>`/`as_string` depending on the actual returned type. [expert]
- [ ] 4.3 Update `bootstrap.rs::execute_exasol_script` so it consumes the new async `execute` via `handle.block_on`.
- [ ] 5.1 In `src/main.rs`, pass the `Runtime`'s `Handle` into `run_interactive_bootstrap` after the runtime is constructed; remove the top-level `tokio::runtime::Builder` only if it can be replaced by `#[tokio::main]` without losing the 16 MiB worker stack — otherwise keep the explicit builder and forward `runtime.handle().clone()` into bootstrap.
- [ ] 6.1 Update `specs/operations/service-runtime/spec.md` `Background` once the change ships so the documented transport is `exarrow-rs` rather than `tungstenite`. (Permanent-spec edits happen during `speq record`, not in this plan.)
- [ ] 7.1 Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all` from a Linux host that can reach the Exasol Personal instance used for integration tests.

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A (Cargo + new exasol.rs) | 1.1, 1.2, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7 |
| Group B (pg_server.rs Arrow plumbing) | 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9 |
| Group C (bootstrap + main wiring) | 4.1, 4.2, 4.3, 5.1 |
| Group D (lints + tests) | 7.1 |

Sequential dependencies:
- Group A → Group B (the pgwire layer cannot compile until `ExasolOutcome` exists).
- Group A → Group C (bootstrap needs the new `ExasolSession::connect` signature).
- Groups B and C are independent and MAY run in parallel after Group A lands.
- Groups B + C → Group D (the workspace must compile before the lint/test sweep).

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| Module | `src/exasol.rs` — `ExaStream`, `Endpoint::connect_stream`, `verify_fingerprint`, `certificate_sha256_hex`, `encrypt_password`, `parse_result`, `parse_result_set`, `parse_columns`, `transpose_data`, `value_to_text`, `response_text_from_message`, `read_json_response`, `request` | Replaced by `exarrow-rs::Connection` and its internal protocol. |
| Type | `src/exasol.rs` — `ExasolColumn`, `ExasolResult` | Replaced by `ExasolOutcome` carrying Arrow. |
| Test | `src/exasol.rs` — `skips_exasol_pong_progress_frame`, `accepts_text_response_frame` | Test the deleted WebSocket frame parser. |
| Variant | `src/pg_server.rs` — `GatewayResponse::TypedQuery { columns, rows: Vec<Vec<Option<String>>>, command_tag }` | Superseded by `GatewayResponse::ArrowQuery`. |
| Function | `src/pg_server.rs` — `query_response_typed` (and its `Vec<Vec<Option<String>>>` parameter shape) | Superseded by `query_response_arrow`. |
| Dependencies | `Cargo.toml` — `tungstenite`, `native-tls`, `rsa`, `sha2`, `base64` | No remaining users after `src/exasol.rs` is rewritten. |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| protocol/read-only-query-path — Client credentials are passed to Exasol | Integration | `tests/exasol_session_integration.rs` (new) | `client_credentials_authenticate_through_exarrow` |
| protocol/read-only-query-path — User runs the simplest smoke-test query | Integration | `tests/smoke_query_integration.rs` (new) | `select_one_round_trips_arrow_through_pgwire` |
| protocol/read-only-query-path — Result values traverse the gateway as Apache Arrow record batches | Integration | `tests/pgwire_arrow_rendering.rs` (new) | `record_batches_render_into_pgwire_data_rows` |
| protocol/read-only-query-path — Exasol session calls are awaited on the Tokio runtime | Integration | `tests/exasol_session_integration.rs` (new) | `concurrent_clients_serialize_through_tokio_mutex` |
| protocol/read-write-statement-path — Supported DML returns command completion | Integration | `tests/dml_command_completion.rs` (new) | `update_returns_exasol_row_count_through_arrow_outcome` |
| protocol/read-write-statement-path — search_path open failure surfaces as a PostgreSQL-compatible error | Integration | `tests/search_path_integration.rs` (extend existing if present, else new) | `set_search_path_to_missing_schema_returns_pg_error` |
| protocol/read-write-statement-path — Cursors materialise Arrow record batches from Exasol | Integration | `tests/cursor_arrow_materialization.rs` (new) | `declare_then_fetch_streams_record_batches_to_client` |
| operations/service-runtime — Operator configures Exasol connectivity | Integration | `tests/config_to_connection_params.rs` (new) | `exasol_config_maps_to_exarrow_connection_params` |
| operations/service-runtime — Operator pins the Exasol certificate by SHA-256 fingerprint | Integration | `tests/tls_fingerprint_integration.rs` (new) | `matching_fingerprint_connects_mismatched_fingerprint_rejected` |
| operations/service-runtime — Operator disables Exasol certificate validation | Integration | `tests/tls_fingerprint_integration.rs` (new) | `nocertcheck_disables_validation_with_warning_log` |
| operations/service-runtime — Operator parses a fingerprint embedded in the Exasol DSN | Unit | `src/exasol.rs::tests` | `dsn_fingerprint_propagates_to_connection_params` |

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| protocol/read-only-query-path | `cargo run -- --config config/local.toml --no-bootstrap` then `psql -h 127.0.0.1 -p 15432 -U <user> -c "SELECT 1"` | psql prints `?column? = 1` and `(1 row)`; server log shows `executed via exarrow-rs` and no `spawn_blocking` frames in tracing. |
| protocol/read-write-statement-path | From `psql`: `BEGIN; UPDATE <table> SET v = v + 1 WHERE k = 1; COMMIT;` followed by `DECLARE c CURSOR FOR SELECT id, name FROM <table>; FETCH 5 FROM c; CLOSE c;` | psql prints `UPDATE 1` and `FETCH 5`; tracing shows the cursor materialised the result as one or more `RecordBatch` values before serving FETCH. |
| operations/service-runtime | With `exasol.certificate_fingerprint` set to the wrong value: `cargo run -- --config config/bad-fingerprint.toml --no-bootstrap`. Then with the correct fingerprint: same command. | First run fails at first client connection with a clear `ExasolError::Connection` message naming the fingerprint mismatch. Second run accepts client connections and `psql` `SELECT 1` succeeds. |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build --release` | Exit 0 |
| Test | `cargo test --all` | 0 failures |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | 0 errors/warnings |
| Format | `cargo fmt --all -- --check` | No changes |
