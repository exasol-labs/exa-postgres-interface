# Tasks: change-exarrow-transport

## Group A (Cargo + new exasol.rs)

- [x] 1.1 Update `Cargo.toml`: add `exarrow-rs = "0.12"` (with `default-features = false` + `features = ["native"]`) and `arrow = "57.1"`; remove `tungstenite`, `native-tls`, `rsa`, `sha2`, `base64`; run `cargo update -p exarrow-rs` and confirm the lockfile resolves.
- [x] 1.2 Verify `exarrow-rs` resolves `arrow` to a single shared version with the gateway and that the `aws-lc-rs` rustls backend it pulls in does not conflict with `pgwire`'s `server-api-aws-lc-rs` feature.
- [x] 2.1 Rewrite `src/exasol.rs`: keep `ExasolError`, replace `ExasolColumn`/`ExasolResult` with `ExasolOutcome { Rows(Vec<RecordBatch>), RowCount(i64) }`, and define `ExasolSession { inner: exarrow_rs::Connection }`.
- [x] 2.2 Implement `ExasolSession::connect(config, username, password)` async: build `exarrow_rs::ConnectionParams` via its `ConnectionBuilder`, apply host/port (from existing DSN parsing), username, password, schema, `use_tls`, `validate_server_certificate`, and `certificate_fingerprint`. Map driver errors into `ExasolError`. [expert]
- [x] 2.3 Reuse the existing `Endpoint::parse` precedence so `ExasolConfig.certificate_fingerprint` overrides a DSN-embedded fingerprint, and so `validate_certificate = false` sets `validate_server_certificate = false` on `exarrow-rs` `ConnectionParams`. [expert]
- [x] 2.4 Implement `ExasolSession::initialize(&mut self, &[String], &str)` async; rewrite `{script}` placeholder substitution and call `Connection::execute_update` for each statement.
- [x] 2.5 Implement `ExasolSession::execute(&mut self, &str)` async: call `Connection::execute(...)`, branch on `ResultSet::row_count()` vs `ResultSet::fetch_all()` and return the matching `ExasolOutcome` arm.
- [x] 2.6 Implement `Drop` / explicit `close`: provide `async fn close(self)` calling `Connection::close().await`; keep a best-effort `Drop` that logs but cannot `.await`.
- [x] 2.7 Rewrite `src/exasol.rs` unit tests: keep `appends_nocertcheck_policy_from_config` and `preserves_dsn_fingerprint` against the new `Endpoint`-to-`ConnectionParams` adapter; delete the `Message::Pong`/`Message::Text` tests.

## Group B (pg_server.rs Arrow plumbing) — blocked by Group A

- [x] 3.1 Change `SessionState.exasol` from `std::sync::Mutex<ExasolSession>` to `tokio::sync::Mutex<ExasolSession>`; do the same for `extended_results`, `cursors`, and `current_schema`.
- [x] 3.2 Remove every `task::spawn_blocking` wrapping an Exasol call; replace each call site with `session.lock().await.execute(&sql).await`. Update the error mapping closures accordingly. [expert]
- [x] 3.3 Replace `ExasolResult` consumers in `pg_server.rs` with `ExasolOutcome` consumers: `execute_exasol_sql`, `execute_client_sql`, `execute_exasol_query`, `fetch_query_rows`, `declare_cursor`, `execute` (StatementPlan::Execute), and `map_exasol_result`.
- [x] 3.4 Define `GatewayResponse::ArrowQuery { schema: SchemaRef, batches: Vec<RecordBatch>, command_tag }` variant replacing `TypedQuery`; update every constructor + `TryInto<Response>` arm. [expert]
- [x] 3.5 Implement Arrow-to-pgwire renderer: `query_response_arrow(schema, batches, command_tag)` that builds `FieldInfo` per Arrow `Field` and streams rows by downcasting each `ArrayRef` through `DataRowEncoder` in text format. Cover Arrow types for Exasol `BOOLEAN`, `DECIMAL`, `DOUBLE`, `DATE`, `TIMESTAMP`, `TIMESTAMP WITH LOCAL TIME ZONE`, `VARCHAR/CHAR/HASHTYPE`. [expert]
- [x] 3.6 Rebuild `GatewayCursor`: store `schema: SchemaRef` + `batches: Vec<RecordBatch>`; rewrite `forward`/`backward`/`absolute`/`relative`/`apply` to operate on a flat row index across batches; render fetched slices through the Arrow-to-pgwire renderer. [expert]
- [x] 3.7 Rewrite `fetch_query_rows` as `arrow_batches_to_text_rows(batches: Vec<RecordBatch>) -> Vec<Vec<Option<String>>>` helper for metadata code; wire path goes through the new renderer.
- [x] 3.8 Update `map_exasol_columns` / `pg_type_for_exasol_data_type` to consume Arrow `DataType` + `Field` metadata instead of Exasol `dataType` JSON, keeping existing OID mapping policy.
- [x] 3.9 Refresh `#[cfg(test)] mod tests` in `pg_server.rs`: rebuild fixtures for `GatewayResponse::ArrowQuery` (small `RecordBatch` literals) so `map_exasol_result_*` and `query_response_arrow` round-trip tests pass.

## Group C (bootstrap + main wiring) — blocked by Group A

- [x] 4.1 In `src/bootstrap.rs`, take a `tokio::runtime::Handle` from `main.rs::run` and call `handle.block_on(ExasolSession::connect(...))` and `handle.block_on(session.execute(...))` from the synchronous prompt flow. Keep terminal I/O synchronous.
- [x] 4.2 Replace `ExasolResult` usage in `bootstrap.rs::first_count` with `ExasolOutcome::Rows(batches)` → read first row, first column as `i64` via Arrow `as_primitive::<Int64Type>` / `as_string`. [expert]
- [x] 4.3 Update `bootstrap.rs::execute_exasol_script` so it consumes the new async `execute` via `handle.block_on`.
- [x] 5.1 In `src/main.rs`, pass the `Runtime`'s `Handle` into `run_interactive_bootstrap` after the runtime is constructed; preserve the 16 MiB worker stack or use `#[tokio::main]` if safe.

## Group D (lints + tests) — blocked by Groups B and C

- [x] 7.1 Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all` from a Linux host; fix any issues found.
