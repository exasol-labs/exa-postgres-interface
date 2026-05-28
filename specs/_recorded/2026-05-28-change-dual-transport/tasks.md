# Tasks: change-dual-transport

## Group A: Config + Dependencies
- [x] 1.1 Add `DEFAULT_TRANSPORT` constant and `transport` field to `ExasolConfig` in `src/config.rs`; add `Transport` enum with `from_config` parser
- [x] 1.2 Update `AppConfig::from_file` to call `Transport::from_config` and fail on unknown values
- [x] 1.3 Add config unit tests: default-when-omitted, explicit `"websocket"`, explicit `"arrow"`, unknown-value rejection
- [x] 2.1 Add Cargo dependencies: `tokio-tungstenite`, `rsa`, `sha2`, `base64`; run `cargo update`

## Group B: Transport Trait + Impls
- [x] 3.1 Define `#[async_trait] ExasolTransport` trait in `src/exasol.rs` [expert]
- [x] 3.2 Restore `ExasolColumn`, widen `ExasolOutcome` (rename `Rows` → `ArrowRows`, add `TypedRows`), migrate all references
- [x] 3.3 Refactor `ExasolSession` to hold `Box<dyn ExasolTransport>`; branch `connect` on `Transport::from_config`
- [x] 3.4 Implement `ArrowTransport` in `src/exasol/arrow_transport.rs`
- [x] 3.5 Implement `WebSocketTransport` in `src/exasol/websocket_transport.rs` (async port via tokio-tungstenite) [expert]
- [x] 3.6 Implement `WebSocketTransport::execute` branching on Exasol JSON `responseData.resultType` [expert]
- [x] 3.7 Build transport-neutral `EndpointConnection` adapter in `src/exasol.rs`
- [x] 3.8 Map `EndpointConnection` into `exarrow_rs::ConnectionParams` in `ArrowTransport::connect`
- [x] 3.9 Implement `WebSocketTransport::connect` with tokio-rustls, fingerprint verification, NOCERTCHECK [expert]
- [x] 3.10 Implement `WebSocketTransport::execute_update` and `WebSocketTransport::close`
- [x] 3.11 Add unit tests: WS frame parsing, `encrypt_password`, `verify_fingerprint`, `pg_type_for_exasol_data_type`

## Group C: pg_server Response/Cursor Restoration
- [x] 4.1 Restore `GatewayResponse::TypedQuery` variant in `src/pg_server.rs`
- [x] 4.2 Restore `query_response_typed` and `pg_type_for_exasol_data_type` in `src/pg_server.rs` [expert]
- [x] 4.3 Update `map_exasol_result` to match new `ExasolOutcome` variants
- [x] 4.4 Add `TypedQuery` arm in `TryInto<Response> for GatewayResponse`
- [x] 4.5 Reshape `GatewayCursor` with `CursorData` enum; rewrite forward/backward/absolute/relative/apply [expert]
- [x] 4.6 Update `declare_cursor` and `CursorPlan` arm to construct matching `CursorData` variant
- [x] 4.7 Update `fetch_query_rows` for `ExasolOutcome::TypedRows` and `ExasolOutcome::ArrowRows`
- [x] 4.8 Add `TypedQuery` round-trip test fixtures in `pg_server.rs` tests

## Group D: Parameterised Test Matrix
- [x] 5.1 Add `tests/common/transport_matrix.rs` with `for_each_transport` helper [expert]
- [x] 5.2 Parameterise `tests/exasol_session_integration.rs` over both transports
- [x] 5.3 Parameterise `tests/smoke_query_integration.rs` over both transports
- [x] 5.4 Parameterise `tests/dml_command_completion.rs` over both transports
- [x] 5.5 Parameterise `tests/search_path_integration.rs` over both transports
- [x] 5.6 Update `tests/cursor_arrow_materialization.rs`; add `cursor_typed_materialization.rs` peer [expert]
- [x] 5.7 Update `tests/config_to_connection_params.rs` for transport variants + unknown-value rejection
- [x] 5.8 Update `tests/tls_fingerprint_integration.rs` to run under both transports
- [x] 5.9 Update `tests/pgwire_arrow_rendering.rs` → rename to `tests/pgwire_rendering.rs`; add WS assertions
- [x] 6.1 Add `tests/transport_selection.rs` covering default, explicit websocket/arrow, and unknown transport

## Group E: Verification
- [x] 7.1 Run `cargo fmt --all`, `cargo clippy`, and `cargo test --all`
- [x] 7.2 Manual smoke test on both transports

## Group F: Code Review Fixes
- [x] 8.1 Remove stale `#![allow(dead_code)]` from `src/config.rs`, `src/exasol/mod.rs`, `src/metadata.rs`
- [x] 8.2 Delete unused `for_each_transport` helper in `tests/common/transport_matrix.rs` and its re-export
- [x] 8.3 Delete dead `currentSchema` no-op branch in `WebSocketTransport::login` (`src/exasol/websocket_transport.rs:86-88`)
- [x] 8.4 Make `WebSocketTransport::execute_update` reject `TypedRows` outcomes to match `ArrowTransport`'s contract
- [x] 8.5 Inline `parse_result_with` into `WebSocketTransport::execute` (single call site)
- [x] 8.6 Drop redundant `TestPkcs1v15Encrypt` alias in `websocket_transport.rs` tests
- [x] 8.7 Install `aws_lc_rs` crypto provider explicitly in `main.rs` startup
- [x] 8.8 Update README — remove stale "Upcoming" reference to `change-exarrow-transport`, document `exasol.transport` config
