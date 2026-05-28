# Plan: Change Dual Transport

## Summary

Reintroduce the previously-deleted hand-rolled Exasol WebSocket JSON transport alongside the current `exarrow-rs` Apache Arrow transport, place both behind a uniform `ExasolTransport` async trait, and add a config-driven, restart-only selection (`exasol.transport`) whose default is encoded as a single `DEFAULT_TRANSPORT` constant in `src/config.rs`.

## Design

### Context

`exarrow-rs` is an experimental Exasol Labs project. The Exasol WebSocket JSON API is the officially supported interface that every other Exasol driver targets. The `change-exarrow-transport` migration deleted the hand-rolled WebSocket transport (and its `ExasolColumn`/`ExasolResult` types, `Vec<Vec<Option<String>>>` row shape, and `GatewayResponse::TypedQuery` wire-mapping) outright, leaving the gateway dependent on a single experimental driver for production-bound deployments.

This plan restores the WebSocket transport (as the new default), keeps the Arrow transport intact, and unifies both under a single async trait. The selection is purely configuration-driven: a restart switches transports; there is no runtime fallback.

- **Goals**
    - Restore the hand-rolled WebSocket JSON transport as an async-only path (no `tungstenite` blocking I/O), reusing the deleted logic ported to `tokio-tungstenite`.
    - Define an `ExasolTransport` async trait with `ArrowTransport` and `WebSocketTransport` implementations.
    - Add `exasol.transport` to `ExasolConfig` (values `"websocket"` | `"arrow"`) with a `DEFAULT_TRANSPORT` constant in `src/config.rs` set to `"websocket"`.
    - Restore `GatewayResponse::TypedQuery` and the typed string-row `GatewayCursor` variant so the WebSocket transport's native shape reaches the wire layer without an Arrow round-trip.
    - Restore the JSON-based `pg_type_for_exasol_data_type` column-OID mapping for the WebSocket path.
    - Apply the existing TLS fingerprint pinning / `NOCERTCHECK` policy and DSN-fingerprint parsing identically to both transports.
- **Non-Goals**
    - Runtime fallback between transports (operator must edit config + restart).
    - Per-connection transport selection. The whole server picks one.
    - Removing or deprecating `exarrow-rs`. It stays as a first-class transport.
    - Changing the wire-protocol contract observable from the client. Identical user-visible behaviour is a requirement, not a goal of the design.
    - Refactoring `metadata.rs` or `policy.rs`.
    - New Exasol features (prepared statements server-side, bulk import/export, etc.).

### Decision

#### Architecture

```
                                ┌─────────────────────────────────┐
                                │           pg_server.rs          │
                                │                                 │
                                │  ExasolPgWireHandler            │
                                │  SessionState {                 │
                                │    exasol: tokio::sync::Mutex<  │
                                │              ExasolSession>     │
                                │  }                              │
                                │                                 │
                                │  GatewayResponse::ArrowQuery    │
                                │  GatewayResponse::TypedQuery    │
                                │  GatewayCursor { data: Arrow    │
                                │                | Typed }        │
                                └────────────────┬────────────────┘
                                                 │ .await
                                                 ▼
                            ┌────────────────────────────────────────┐
                            │              src/exasol.rs             │
                            │                                        │
                            │  pub struct ExasolSession {            │
                            │    inner: Box<dyn ExasolTransport>     │
                            │  }                                     │
                            │                                        │
                            │  #[async_trait] pub trait              │
                            │      ExasolTransport: Send {           │
                            │    async fn execute(&mut self, &str)   │
                            │        -> Result<ExasolOutcome, _>;    │
                            │    async fn execute_update(&mut self,  │
                            │        &str) -> Result<(), _>;         │
                            │    async fn close(self: Box<Self>)     │
                            │        -> Result<(), _>;               │
                            │  }                                     │
                            │                                        │
                            │  ExasolOutcome::ArrowRows(Vec<RB>)     │
                            │  ExasolOutcome::TypedRows{cols,rows}   │
                            │  ExasolOutcome::RowCount(i64)          │
                            └──────────┬──────────────────┬──────────┘
                                       │                  │
                                       ▼                  ▼
                          ┌──────────────────┐  ┌──────────────────────┐
                          │  ArrowTransport  │  │  WebSocketTransport  │
                          │  (exarrow_rs::   │  │  (tokio-tungstenite, │
                          │   Connection)    │  │   rustls, rsa+sha2)  │
                          └──────────────────┘  └──────────────────────┘
```

`ExasolSession` owns a `Box<dyn ExasolTransport>`. The selection happens once in `ExasolSession::connect(config, user, pw)`, which reads `config.transport` and constructs the matching impl. Every consumer (`pg_server.rs`, `bootstrap.rs`) calls `session.execute(sql).await` exactly as it does today.

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| `#[async_trait] ExasolTransport` async trait with `Box<dyn ExasolTransport>` | `src/exasol.rs` | Single dispatch point for both impls; one call site in `pg_server.rs`; matches existing `async-trait = "0.1"` dependency already in use. |
| Outcome enum widened with two row-set variants | `ExasolOutcome::ArrowRows(Vec<RecordBatch>)` / `TypedRows { columns, rows }` / `RowCount(i64)` | Preserves the transport's native row shape without an intermediate re-encoding. `RowCount` unifies to `i64` (the WS transport's `usize` is cast at the boundary). |
| `GatewayResponse` carries two row-set variants | `GatewayResponse::ArrowQuery { schema, batches, command_tag }` (retained) + `GatewayResponse::TypedQuery { columns, rows, command_tag }` (restored) | Wire-layer renderer matches on the response variant. Restored variant is the pre-migration shape from git history (`db58cf5` / earlier). |
| `GatewayCursor` carries an internal enum | `enum CursorData { Arrow { schema, batches } / Typed { columns, rows } }` inside one `GatewayCursor` struct | Cursor stepping (`position`, `scroll`, `hold`) is transport-agnostic; only the materialised payload varies. A single `GatewayCursor` struct keeps `HashMap<String, GatewayCursor>` and avoids `Box<dyn Cursor>`. |
| Transport-agnostic certificate policy | `Endpoint::parse(...)` returns a transport-neutral `EndpointConnection { host, port, encryption, validate_certificate, fingerprint }` that each transport adapts to its own connection parameters | Identical fingerprint / `NOCERTCHECK` / DSN-suffix precedence across both transports. |
| `DEFAULT_TRANSPORT` named const | `src/config.rs::DEFAULT_TRANSPORT: &str = "websocket"` plus `#[serde(default = "default_transport")]` on `ExasolConfig.transport` | Changing the default is a one-line, git-diffable PR. |
| Bootstrap uses the configured transport | `bootstrap.rs::run_interactive_bootstrap` opens its `ExasolSession` through `ExasolSession::connect(&config.exasol, ...)` exactly as the gateway does | Install-time verification exercises the path the gateway will actually use; no risk of a passing bootstrap masking a broken gateway transport. |
| Parameterised integration-test harness | `tests/common/transport_matrix.rs` exposes `for_each_transport(|tcfg| { ... })` so every test runs against both transports | Single source of truth for the integration matrix; no per-transport test duplication. |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| Add `ExasolTransport` async trait behind `Box<dyn ExasolTransport>` | (a) Two-variant enum (`enum ExasolSessionImpl { Arrow(...), WebSocket(...) }`) dispatched by match arms. (b) Generic `ExasolSession<T: ExasolTransport>` with monomorphisation. | (a) duplicates every method's match arm and forces every consumer to acknowledge both transports. (b) leaks the transport choice into every type signature in `pg_server.rs` and forces `SessionState` to be generic, which conflicts with `Arc<dyn PgWireServerHandlers>`. The trait keeps a single call site and a single `SessionState`. |
| Default to WebSocket | Default to Arrow (status quo). | The user intent is explicit: the WS JSON API is the officially supported Exasol interface; `exarrow-rs` is labs. Production resilience requires the supported path as default. |
| Encode default as `pub const DEFAULT_TRANSPORT: &str` rather than `#[derive(Default)]` on an enum | Use a `Transport` enum with `#[default] WebSocket`. | A `const` is the simplest one-line git diff and lives in the same file the operator already edits to change other defaults. An enum-with-default works but spreads the change across the type + the serde `#[serde(default)]` function. The const is strictly less surface area. |
| `ExasolOutcome::RowCount(i64)` (unified) | Keep `RowCount(usize)` for the WS path and `RowCount(i64)` for the Arrow path as two variants. | The PostgreSQL command-completion machinery already handles a single integer width; carrying two variants forces every consumer to handle both. Casting `usize -> i64` at the WS transport boundary is one expression. |
| `GatewayCursor` carries an internal enum (`CursorData::{Arrow, Typed}`), single struct | (a) Two concrete types (`ArrowCursor`, `TypedCursor`) behind a `trait Cursor`. (b) Parallel `HashMap<String, ArrowCursor>` + `HashMap<String, TypedCursor>` registries. | (a) forces dyn dispatch and an extra heap allocation per cursor; (b) requires every cursor operation (`CLOSE`, `FETCH`, `MOVE`) to look in two maps. The internal enum keeps `position`/`scroll`/`hold` shared and isolates the variant only inside the rendering path. |
| Parameterised test matrix in `tests/common/transport_matrix.rs` | (a) Duplicate every existing integration test file under a `_websocket` suffix. (b) Run integration tests only under one transport and trust manual testing for the other. | (a) doubles the test files for a single-axis variation. (b) leaves the unselected transport unverified, which is exactly the risk this plan exists to mitigate. The harness lets each test parameterise over both transports with a few lines of boilerplate. |
| Bootstrap uses the configured transport (not always Arrow) | Always use Arrow for bootstrap, regardless of `exasol.transport`. | Bootstrap is the operator's first end-to-end check that their config works. If bootstrap silently used a different transport from the gateway, a transport-specific config error (wrong fingerprint format, TLS mismatch) would only surface after the gateway is already serving traffic. Reusing the configured transport keeps install-time verification honest. |
| Restore `GatewayResponse::TypedQuery` and JSON-based OID mapping verbatim from git history (commit `db58cf5` and earlier) | Build an Arrow shim that converts the WS `Vec<Vec<Option<String>>>` shape into `RecordBatch` on the WS boundary so `pg_server.rs` only knows Arrow. | The user intent is explicit: when the WS transport is active the pipeline SHALL match the pre-migration pipeline byte-for-byte (typed columns, string rows, `TypedQuery` variant, JSON `dataType -> OID` mapping). An Arrow shim would still leave the WS transport as a second-class path with subtly different OID mapping. |
| Async-port the deleted WS transport (use `tokio-tungstenite`) rather than restore it as a sync transport wrapped in `spawn_blocking` | Restore the original `tungstenite` sync code and use `task::spawn_blocking` around every call. | The Arrow-transport migration explicitly removed every `spawn_blocking` Exasol call site. Restoring sync I/O would re-introduce the locking constraints (`std::sync::Mutex`, no guard across `.await`) the previous plan deleted. `tokio-tungstenite` is the canonical async port and lets the WS impl satisfy the same `async fn execute` contract as Arrow. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| operations/service-runtime | CHANGED | `specs/_plans/change-dual-transport/operations/service-runtime/spec.md` |
| protocol/read-only-query-path | CHANGED | `specs/_plans/change-dual-transport/protocol/read-only-query-path/spec.md` |
| protocol/read-write-statement-path | CHANGED | `specs/_plans/change-dual-transport/protocol/read-write-statement-path/spec.md` |

## Dependencies

Cargo additions:

| Crate | Reason |
|-------|--------|
| `tokio-tungstenite = "0.24"` (with `rustls-tls-native-roots` feature) | Async WebSocket client for the restored WS transport. Replaces the previously-removed sync `tungstenite`. |
| `rsa = "0.9"` | Exasol login encrypts the password with the server-provided RSA public key. Restored from the pre-migration `Cargo.toml`. |
| `sha2 = "0.10"` | Computes the SHA-256 fingerprint of the server certificate's DER encoding for fingerprint pinning. Restored from the pre-migration `Cargo.toml`. |
| `base64 = "0.22"` | Encodes the RSA-encrypted password for the Exasol login JSON envelope. Restored from the pre-migration `Cargo.toml`. |
| `serde_json = "1"` | Parses Exasol's JSON request/response envelopes. (Already pulled in transitively, but added as a direct dep so the WS transport owns its shape.) |

Cargo crates that stay (no change):

| Crate | Role |
|-------|------|
| `exarrow-rs = "0.12"` | Arrow transport. Untouched. |
| `arrow = "57.1"` | `RecordBatch` rendering for the Arrow path. |
| `tokio-rustls = "0.26"` | TLS for both transports. The WS transport uses `tokio-rustls` rather than re-introducing `native-tls`. |
| `async-trait = "0.1"` | Powers the `ExasolTransport` trait. |

Cargo crates removed: none (this plan adds transport breadth, it does not remove the Arrow path).

## Migration

| Current | New |
|---------|-----|
| `ExasolSession { inner: exarrow_rs::Connection }` | `ExasolSession { inner: Box<dyn ExasolTransport> }` |
| `ExasolOutcome { Rows(Vec<RecordBatch>), RowCount(i64) }` | `ExasolOutcome { ArrowRows(Vec<RecordBatch>), TypedRows { columns: Vec<ExasolColumn>, rows: Vec<Vec<Option<String>>> }, RowCount(i64) }` (existing `Rows` renamed to `ArrowRows`) |
| `pub async fn ExasolSession::connect(config, user, pw)` always builds Arrow | `pub async fn ExasolSession::connect(config, user, pw)` reads `config.transport` and constructs `ArrowTransport` or `WebSocketTransport` |
| `ExasolConfig { dsn, encryption, certificate_fingerprint, validate_certificate, pass_client_credentials, schema }` | `ExasolConfig { dsn, encryption, certificate_fingerprint, validate_certificate, pass_client_credentials, schema, transport }` with `#[serde(default = "default_transport")]` returning `DEFAULT_TRANSPORT.to_owned()` |
| `GatewayResponse::ArrowQuery { schema, batches, command_tag }` (only row-returning variant) | `GatewayResponse::{ ArrowQuery { schema, batches, command_tag }, TypedQuery { columns: Vec<ExasolColumn>, rows: Vec<Vec<Option<String>>>, command_tag } }` |
| `GatewayCursor { schema, batches, position, scroll, hold }` | `GatewayCursor { data: CursorData, position, scroll, hold }` where `enum CursorData { Arrow { schema, batches }, Typed { columns, rows } }` |
| `query_response_arrow(schema, batches, command_tag)` (sole row renderer) | `query_response_arrow(...)` + restored `query_response_typed(columns, rows, command_tag)` (verbatim from git history at commit `db58cf5`) |
| `pg_type_for_arrow_field(field) -> Type` (sole column-OID mapper) | `pg_type_for_arrow_field(...)` + restored `pg_type_for_exasol_data_type(json: &serde_json::Value) -> Type` for the WS path |
| `bootstrap.rs::ExasolSession::connect(&config.exasol, ...)` (Arrow-only) | Unchanged call site; `ExasolSession::connect` now selects the transport internally |
| No `tests/common/transport_matrix.rs` | New helper `tests/common/transport_matrix.rs` exposing `fn for_each_transport(test: impl Fn(TransportTestConfig))` used by every integration test in `tests/` |

## Implementation Tasks

- [ ] 1.1 Add `DEFAULT_TRANSPORT: &str = "websocket"` constant and `transport: String` field (with `#[serde(default = "default_transport")]`) to `ExasolConfig` in `src/config.rs`; add a `Transport` enum (`Transport::WebSocket | Transport::Arrow`) with a `from_config(&ExasolConfig) -> Result<Transport, ConfigError>` parser that rejects unknown values.
- [ ] 1.2 Update `AppConfig::from_file` to call `Transport::from_config(&config.exasol)` so unknown transport values fail startup with a clear error naming the accepted values.
- [ ] 1.3 Add config unit tests in `src/config.rs` for default-when-omitted, explicit `"websocket"`, explicit `"arrow"`, and the unknown-value rejection path.
- [ ] 2.1 Add Cargo dependencies: `tokio-tungstenite = "0.24"` (with `rustls-tls-native-roots`), `rsa = "0.9"`, `sha2 = "0.10"`, `base64 = "0.22"`, `serde_json = "1"`; run `cargo update` and confirm `aws-lc-rs` (already pulled by pgwire) does not conflict with the `rustls-tls-native-roots` feature.
- [ ] 3.1 In `src/exasol.rs`, define `#[async_trait] pub(crate) trait ExasolTransport: Send` with `async fn execute(&mut self, sql: &str) -> Result<ExasolOutcome, ExasolError>`, `async fn execute_update(&mut self, sql: &str) -> Result<(), ExasolError>`, and `async fn close(self: Box<Self>) -> Result<(), ExasolError>`. [expert]
- [ ] 3.2 In `src/exasol.rs`, restore `pub struct ExasolColumn { name: String, data_type: serde_json::Value }` and widen `ExasolOutcome` to `{ ArrowRows(Vec<RecordBatch>), TypedRows { columns: Vec<ExasolColumn>, rows: Vec<Vec<Option<String>>> }, RowCount(i64) }`; rename the current `Rows` variant to `ArrowRows` and migrate all references.
- [ ] 3.3 Refactor `src/exasol.rs::ExasolSession` to hold `inner: Box<dyn ExasolTransport>`; reshape `ExasolSession::connect` to branch on `Transport::from_config(...)` and construct the appropriate transport impl.
- [ ] 3.4 Implement `ArrowTransport { conn: exarrow_rs::Connection }` impl of `ExasolTransport` in `src/exasol/arrow_transport.rs`; this is the existing Arrow code factored out behind the trait. Returns `ExasolOutcome::ArrowRows` and `ExasolOutcome::RowCount(i64)`.
- [ ] 3.5 Implement `WebSocketTransport { ws: WebSocketStream<...>, session_id: u64, ... }` in `src/exasol/websocket_transport.rs`, ported from the pre-migration code (commit history before `change-exarrow-transport`) onto `tokio-tungstenite`. Restores: `ExaStream` (async), `verify_fingerprint`, `certificate_sha256_hex`, `encrypt_password`, `parse_result`, `parse_result_set`, `parse_columns`, `transpose_data`, `value_to_text`, `response_text_from_message`, `read_json_response`, `request`. [expert]
- [ ] 3.6 In `WebSocketTransport::execute`, branch the Exasol JSON `responseData` envelope on `resultType`: `resultSet` → `ExasolOutcome::TypedRows { columns, rows }`; `rowCount` → `ExasolOutcome::RowCount(rowCount as i64)`. Reuse the restored helpers from 3.5. [expert]
- [ ] 3.7 Build a transport-neutral `EndpointConnection { host, port, encryption, validate_certificate, fingerprint: Option<String> }` adapter in `src/exasol.rs` from `Endpoint::parse(&config.dsn, &config)`; both transport impls consume this adapter so DSN-fingerprint precedence and `NOCERTCHECK` behave identically.
- [ ] 3.8 In `ArrowTransport::connect`, map `EndpointConnection` into `exarrow_rs::ConnectionParams` (existing logic, moved behind the new boundary).
- [ ] 3.9 In `WebSocketTransport::connect`, use `tokio-rustls` for TLS (no `native-tls`), apply `verify_fingerprint` when `EndpointConnection.fingerprint` is set, and bypass server-cert validation when `validate_certificate == false` and no fingerprint is present. [expert]
- [ ] 3.10 Implement `WebSocketTransport::execute_update` (used for session-init SQL) and `WebSocketTransport::close` (sends Exasol `disconnect` then closes the WS).
- [ ] 3.11 Add focused unit tests in `src/exasol.rs` and the new sub-modules: WS frame parsing for `Pong`/`Text` (restored), `encrypt_password` round-trip, `verify_fingerprint` accept/reject, `pg_type_for_exasol_data_type` for each Exasol JSON `dataType` shape.
- [ ] 4.1 In `src/pg_server.rs`, restore `GatewayResponse::TypedQuery { columns: Vec<ExasolColumn>, rows: Vec<Vec<Option<String>>>, command_tag: Option<String> }` (verbatim from commit history); update the variant list in `enum GatewayResponse`.
- [ ] 4.2 In `src/pg_server.rs`, restore `query_response_typed(columns, rows, command_tag) -> PgWireResult<QueryResponse>` and `pg_type_for_exasol_data_type(json) -> Type` from commit history; keep the existing `query_response_arrow` and `pg_type_for_arrow_field` untouched. [expert]
- [ ] 4.3 In `src/pg_server.rs::map_exasol_result`, match `ExasolOutcome::ArrowRows` → `GatewayResponse::ArrowQuery`, `ExasolOutcome::TypedRows` → `GatewayResponse::TypedQuery`, `ExasolOutcome::RowCount` → `GatewayResponse::Execution`.
- [ ] 4.4 In `src/pg_server.rs::TryInto<Response> for GatewayResponse`, add the `TypedQuery` arm calling `query_response_typed(...)`; keep the `ArrowQuery` arm calling `query_response_arrow(...)`.
- [ ] 4.5 Reshape `struct GatewayCursor` to `{ data: CursorData, position: isize, scroll: bool, hold: bool }` with `enum CursorData { Arrow { schema: SchemaRef, batches: Vec<RecordBatch> }, Typed { columns: Vec<ExasolColumn>, rows: Vec<Vec<Option<String>>> } }`; rewrite `forward`/`backward`/`absolute`/`relative`/`apply` to dispatch on the variant for the rendered slice while keeping the index logic shared. [expert]
- [ ] 4.6 Update `declare_cursor` and the `CursorPlan` execution arm to construct the matching `CursorData` variant from the `ExasolOutcome` returned by the transport.
- [ ] 4.7 Update `fetch_query_rows` (used by metadata code paths) so the `ExasolOutcome::TypedRows` arm reads its string rows directly and the `ExasolOutcome::ArrowRows` arm continues to go through `arrow_batches_to_text_rows`.
- [ ] 4.8 Update existing in-file `#[cfg(test)] mod tests` in `pg_server.rs`: keep the `ArrowQuery` fixtures, add fresh `TypedQuery` round-trip fixtures matching the restored pre-migration tests.
- [ ] 5.1 Add `tests/common/transport_matrix.rs` exposing `pub fn for_each_transport(name: &str, body: impl Fn(TransportTestConfig))` and `pub struct TransportTestConfig { pub transport: &'static str, pub make_app_config: Box<dyn Fn() -> AppConfig> }`; expose it via `tests/common/mod.rs`. [expert]
- [ ] 5.2 Update `tests/exasol_session_integration.rs` to parameterise its scenarios over both transports via `for_each_transport`.
- [ ] 5.3 Update `tests/smoke_query_integration.rs` similarly.
- [ ] 5.4 Update `tests/dml_command_completion.rs` similarly.
- [ ] 5.5 Update `tests/search_path_integration.rs` similarly.
- [ ] 5.6 Update `tests/cursor_arrow_materialization.rs` and split: keep one Arrow-specific assertion (`CursorData::Arrow`) and add a `cursor_typed_materialization.rs` peer for `CursorData::Typed`; both share the `for_each_transport` helper for setup but assert against the variant their transport produces. [expert]
- [ ] 5.7 Update `tests/config_to_connection_params.rs` to cover `transport = "websocket"` + `transport = "arrow"` + the unknown-value rejection path.
- [ ] 5.8 Update `tests/tls_fingerprint_integration.rs` so the matching-fingerprint, mismatched-fingerprint, and `NOCERTCHECK` cases each run under both transports.
- [ ] 5.9 Update `tests/pgwire_arrow_rendering.rs` to assert the WS transport produces `GatewayResponse::TypedQuery` and the Arrow transport produces `GatewayResponse::ArrowQuery`; rename to `tests/pgwire_rendering.rs` to reflect the broader scope.
- [ ] 6.1 Add a `transport-selection` integration test in `tests/transport_selection.rs` covering: (a) default transport selection when `exasol.transport` is omitted matches `DEFAULT_TRANSPORT`; (b) explicit `"websocket"` runs WS path; (c) explicit `"arrow"` runs Arrow path; (d) `transport = "tcp"` fails `AppConfig::from_file` with a clear error.
- [ ] 7.1 Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all` from a Linux host that can reach the Exasol Personal instance.
- [ ] 7.2 Manual smoke test on both transports per the Manual Testing table below; record the observed identical client-side output.

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A (config + dependencies) | 1.1, 1.2, 1.3, 2.1 |
| Group B (transport trait + impls) | 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11 |
| Group C (pg_server response/cursor restoration) | 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8 |
| Group D (parameterised test matrix) | 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9, 6.1 |
| Group E (lints + manual verification) | 7.1, 7.2 |

Sequential dependencies:
- Group A → Group B (transport impls read `Transport::from_config`).
- Group B → Group C (`pg_server.rs` consumes the new `ExasolOutcome` shape).
- Groups B + C → Group D (integration tests need both transports compilable).
- Groups B + C + D → Group E.

Within Group B, tasks 3.5 / 3.6 / 3.9 must run sequentially (each depends on the prior file existing). Tasks 3.7 / 3.8 / 3.10 / 3.11 may run alongside.

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| (none) | — | This plan strictly adds breadth; no symbols deleted. The Arrow path stays intact. Any cleanup of unused glue from `change-exarrow-transport` is deferred to a follow-up plan if needed. |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| operations/service-runtime — Operator configures Exasol connectivity | Integration | `tests/config_to_connection_params.rs` | `exasol_config_maps_to_each_transport_connection_params` |
| operations/service-runtime — Operator selects the WebSocket transport | Integration | `tests/transport_selection.rs` | `explicit_websocket_transport_runs_websocket_path` |
| operations/service-runtime — Operator selects the Arrow transport | Integration | `tests/transport_selection.rs` | `explicit_arrow_transport_runs_arrow_path` |
| operations/service-runtime — Operator supplies an unknown transport value | Unit | `src/config.rs::tests` | `unknown_transport_value_fails_config_load` |
| operations/service-runtime — Switching transports requires a restart | Integration | `tests/transport_selection.rs` | `transport_choice_is_fixed_for_session_lifetime` |
| operations/service-runtime — Operator pins the Exasol certificate by SHA-256 fingerprint | Integration | `tests/tls_fingerprint_integration.rs` | `matching_fingerprint_connects_under_each_transport` |
| operations/service-runtime — Operator disables Exasol certificate validation | Integration | `tests/tls_fingerprint_integration.rs` | `nocertcheck_disables_validation_under_each_transport` |
| operations/service-runtime — Operator parses a fingerprint embedded in the Exasol DSN | Integration | `tests/tls_fingerprint_integration.rs` | `dsn_fingerprint_propagates_under_each_transport` |
| protocol/read-only-query-path — Client credentials are passed to Exasol | Integration | `tests/exasol_session_integration.rs` | `client_credentials_authenticate_under_each_transport` |
| protocol/read-only-query-path — User runs the simplest smoke-test query | Integration | `tests/smoke_query_integration.rs` | `select_one_round_trips_under_each_transport` |
| protocol/read-only-query-path — Result values traverse the gateway in the transport's native shape | Integration | `tests/pgwire_rendering.rs` | `each_transport_emits_its_native_response_variant` |
| protocol/read-only-query-path — Exasol session calls are awaited on the Tokio runtime | Integration | `tests/exasol_session_integration.rs` | `concurrent_clients_serialize_under_each_transport` |
| protocol/read-only-query-path — WebSocket transport returns typed string rows with Exasol type OIDs | Integration | `tests/pgwire_rendering.rs` | `websocket_transport_produces_typed_query_with_json_oids` |
| protocol/read-write-statement-path — Supported DML returns command completion | Integration | `tests/dml_command_completion.rs` | `update_returns_row_count_under_each_transport` |
| protocol/read-write-statement-path — search_path open failure surfaces as a PostgreSQL-compatible error | Integration | `tests/search_path_integration.rs` | `set_search_path_to_missing_schema_returns_pg_error_under_each_transport` |
| protocol/read-write-statement-path — Cursors materialise results in the transport's native shape | Integration | `tests/cursor_arrow_materialization.rs` + `tests/cursor_typed_materialization.rs` | `declare_then_fetch_under_arrow_transport` + `declare_then_fetch_under_websocket_transport` |

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| operations/service-runtime | With `exasol.transport = "websocket"` in `config/local.toml`: `cargo run -- --config config/local.toml --no-bootstrap`. Then change to `exasol.transport = "arrow"` and restart. | Both runs accept the same `psql -h 127.0.0.1 -p 15432 -U <user> -c "SELECT 1"`; server log shows `selected transport: websocket` for the first run and `selected transport: arrow` for the second. |
| protocol/read-only-query-path | From `psql` against a `transport = "websocket"` instance: `SELECT 1`. From `psql` against a `transport = "arrow"` instance: same command. | Both print `?column? = 1` / `(1 row)`. Server log for the WS run shows `GatewayResponse::TypedQuery`; log for the Arrow run shows `GatewayResponse::ArrowQuery`. |
| protocol/read-write-statement-path | From `psql` against each transport in turn: `BEGIN; UPDATE <table> SET v = v + 1 WHERE k = 1; COMMIT;` followed by `DECLARE c CURSOR FOR SELECT id, name FROM <table>; FETCH 5 FROM c; CLOSE c;`. | Both print `UPDATE 1` and `FETCH 5`. WS-transport tracing shows `CursorData::Typed`; Arrow-transport tracing shows `CursorData::Arrow`. |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build --release` | Exit 0 |
| Test | `cargo test --all` | 0 failures |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | 0 errors/warnings |
| Format | `cargo fmt --all -- --check` | No changes |
