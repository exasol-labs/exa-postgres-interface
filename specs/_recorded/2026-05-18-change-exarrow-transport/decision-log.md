# Decision Log: change-exarrow-transport

Date: 2026-05-18

## Interview

**Q:** How far should Arrow format penetrate the stack?
**A:** Full propagation — replace `ExasolResult` with Apache Arrow `RecordBatch` all the way through `pg_server.rs`. No adapter shim that pre-stringifies values inside `src/exasol.rs`.

**Q:** Should certificate fingerprint validation be preserved?
**A:** Yes. The current `certificate_fingerprint` and `validate_certificate` config fields plus the `NOCERTCHECK` / SHA-256 logic must continue to work on top of `exarrow-rs`.

**Q:** Async model?
**A:** Make `ExasolSession` fully async (async/await throughout). No `block_in_place`, no `task::spawn_blocking` wrapping Exasol calls. Update every call site in `pg_server.rs` and `bootstrap.rs` to await directly.

## Design Decisions

### [1] Adopt exarrow-rs as the Exasol transport

- **Decision:** Replace the hand-rolled `tungstenite` + JSON + RSA login transport in `src/exasol.rs` with `exarrow-rs = "0.12"`, using its native TCP transport (`features = ["native"]`).
- **Alternatives:**
    - Keep the existing transport and merely retype results to `RecordBatch` — rejected because it leaves `rsa`, `native-tls`, and the manual protocol loop in place and gives up the maintenance win.
    - Build a thin in-tree async WebSocket client on top of `tokio-tungstenite` plus a custom Arrow decoder — rejected because Exasol Labs already ships and maintains `exarrow-rs`; re-implementing it is gratuitous.
- **Rationale:** `exarrow-rs` is async on Tokio, returns Apache Arrow `RecordBatch` values directly, uses `rustls` + `aws-lc-rs`, and supports certificate fingerprint pinning natively. Adopting it deletes ~400 lines of bespoke protocol code and removes four runtime dependencies (`tungstenite`, `native-tls`, `rsa`, `sha2`, plus `base64`).
- **Promotes to ADR:** yes

### [2] Propagate Arrow RecordBatch through pg_server.rs

- **Decision:** Replace `GatewayResponse::TypedQuery { columns, rows: Vec<Vec<Option<String>>>, command_tag }` with `GatewayResponse::ArrowQuery { schema: arrow::datatypes::SchemaRef, batches: Vec<RecordBatch>, command_tag }` and render Arrow values into pgwire's `DataRowEncoder` at the wire-protocol boundary. The cursor registry stores `Vec<RecordBatch>` instead of pre-stringified rows.
- **Alternatives:** Convert Arrow batches to `Vec<Vec<Option<String>>>` inside `src/exasol.rs` so `pg_server.rs` stays unchanged — rejected per the interview answer because it forces a value-by-value `String` round-trip and loses the throughput benefit.
- **Rationale:** Matches the explicit user intent ("full propagation, no adapter shim"). Gives one central Arrow-to-pgwire renderer to maintain.
- **Promotes to ADR:** yes

### [3] Use exarrow-rs' built-in fingerprint/validation parameters

- **Decision:** Translate `ExasolConfig.certificate_fingerprint`, `validate_certificate`, and the DSN-embedded fingerprint suffix directly into `exarrow_rs::ConnectionParams { certificate_fingerprint, validate_server_certificate, use_tls, ... }`. Keep the existing precedence rule from `Endpoint::parse` (explicit config field wins over DSN suffix; `validate_certificate = false` with no fingerprint maps to `validate_server_certificate = false`).
- **Alternatives:** Build a custom `rustls::ServerCertVerifier` and inject it through an `exarrow-rs` extension hook — rejected because `ConnectionParams` already accepts the fingerprint and validation flag, and reimplementing the verifier locks us into `exarrow-rs` internals.
- **Rationale:** Lowest-friction way to preserve every existing configuration knob without forking driver internals.
- **Promotes to ADR:** yes

### [4] Replace std::sync::Mutex with tokio::sync::Mutex around the Exasol session

- **Decision:** `SessionState.exasol` becomes `tokio::sync::Mutex<ExasolSession>`. The cursor registry, extended-query result cache, and current-schema cell move to `tokio::sync::Mutex` for consistency and to eliminate lock-poisoning handling.
- **Alternatives:** Keep `std::sync::Mutex` and continue wrapping `exarrow-rs` calls in `task::spawn_blocking` — rejected directly by the interview ("no `block_in_place`").
- **Rationale:** A `std::sync::Mutex` guard cannot be held across `.await`, but the new async `execute` requires awaiting the driver while the session lock is held. `tokio::sync::Mutex` is the canonical fix.
- **Promotes to ADR:** yes

### [5] Bootstrap drives async Exasol calls via the runtime handle

- **Decision:** `main.rs` continues to build the multi-thread runtime explicitly (to retain the 16 MiB worker stack). The interactive bootstrap in `bootstrap.rs` keeps a synchronous prompt loop but takes a `tokio::runtime::Handle` and drives `ExasolSession::connect` / `execute` through `handle.block_on(...)`.
- **Alternatives:**
    - Convert `bootstrap.rs` to fully async and call it from `main.rs` after `Runtime::block_on(run())` — possible but requires threading async through every prompt and reading from stdin asynchronously for no behavioural gain.
    - Spin up a second runtime for bootstrap — rejected because it wastes resources and complicates the lifecycle.
- **Rationale:** Keeps terminal I/O synchronous and avoids splitting runtimes; the existing top-level runtime is reused.
- **Promotes to ADR:** no

### [6] Keep a string-based fetch helper for metadata code paths

- **Decision:** Repackage the current `fetch_query_rows -> Vec<Vec<Option<String>>>` helper as `arrow_batches_to_text_rows` (Arrow input, string-rows output) so the metadata layer in `pg_server.rs` that synthesises PostgreSQL catalog rows from Exasol queries keeps working. The wire path goes through `query_response_arrow` directly and bypasses this helper.
- **Alternatives:** Push Arrow all the way into the metadata builders and rewrite every `MetadataPlan::*` arm — rejected as out of scope for this plan; it is a separate, larger refactor.
- **Rationale:** Confines the migration to transport + wire-protocol rendering. The metadata-builder rewrite can land later without re-doing this transport change.
- **Promotes to ADR:** no

### [7] Default to exarrow-rs native TCP transport

- **Decision:** Enable only the `native` feature of `exarrow-rs` for the first cut. WebSocket transport remains available behind a feature flag but is not turned on by default.
- **Alternatives:** Enable the `websocket` feature to match the prior `tungstenite`-based transport surface — possible if the deployed Exasol cluster only accepts WebSocket. The decision is reversible by flipping one Cargo feature.
- **Rationale:** Native TCP is the upstream default and the path the driver authors document as primary. If integration testing reveals an Exasol deployment that refuses the native protocol, the plan permits adding `features = ["websocket"]` with no other changes.
- **Promotes to ADR:** no

## Review Findings

<!-- Populated by speq-implement after code review. -->
