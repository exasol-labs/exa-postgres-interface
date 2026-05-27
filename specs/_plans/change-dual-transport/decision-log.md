# Decision Log: change-dual-transport

Date: 2026-05-21

## Interview

**Q:** What does "easy to change the default transport" mean in practice?
**A:** A named constant in code — a single `DEFAULT_TRANSPORT` const in `src/config.rs` makes the default obvious and git-diffable. Changing the default in a future release is a one-line PR.

**Q:** When the WebSocket transport is active, how should result types flow downstream?
**A:** Same behaviour as before the Arrow migration — the full pre-migration pipeline: proper type OIDs from Exasol's JSON `dataType` metadata, string-row results, the old `TypedQuery` response variant, the old string-row `GatewayCursor`, and the JSON-based OID mapping (`pg_type_for_exasol_data_type`).

**Q:** Automatic fallback between transports at runtime?
**A:** No. Config-driven switch only. Gateway restart required to switch transports.

## Design Decisions

### [1] ExasolTransport async trait behind Box<dyn>

- **Decision:** Define `#[async_trait] pub(crate) trait ExasolTransport: Send` with `execute`, `execute_update`, and `close`. `ExasolSession` holds `inner: Box<dyn ExasolTransport>`; selection happens once in `ExasolSession::connect`.
- **Alternatives:**
    - Two-variant enum (`enum ExasolSessionImpl { Arrow(...), WebSocket(...) }`) dispatched by match arms — rejected because every method duplicates the match and forces every consumer to acknowledge both transports.
    - Generic `ExasolSession<T: ExasolTransport>` monomorphised — rejected because the transport choice would leak into every type signature in `pg_server.rs` (`SessionState` becomes generic, conflicting with `Arc<dyn PgWireServerHandlers>`).
- **Rationale:** One call site in `pg_server.rs` (`session.execute(sql).await`), one `SessionState` type, and a localisable place to add a third transport later.
- **Promotes to ADR:** yes

### [2] Default transport is WebSocket, encoded as a `DEFAULT_TRANSPORT` const

- **Decision:** `pub const DEFAULT_TRANSPORT: &str = "websocket"` in `src/config.rs`. `ExasolConfig.transport` uses `#[serde(default = "default_transport")]` which returns `DEFAULT_TRANSPORT.to_owned()`.
- **Alternatives:**
    - Default to Arrow (current behaviour) — rejected explicitly by user intent: the WS JSON API is the officially supported Exasol interface; `exarrow-rs` is labs.
    - A `Transport` enum with `#[default] WebSocket` — works but spreads the default across the type + the serde `#[serde(default)]` function. The const is strictly less surface area.
- **Rationale:** Production resilience requires the officially supported path as default. The `const` keeps "change the default" to a single, git-diffable line.
- **Promotes to ADR:** yes

### [3] WebSocket path restores the pre-migration shape verbatim (not an Arrow shim)

- **Decision:** When the WS transport is active, the pipeline matches the pre-migration pipeline: `ExasolColumn { name, data_type: serde_json::Value }`, `Vec<Vec<Option<String>>>` row shape, `GatewayResponse::TypedQuery`, `GatewayCursor` carrying typed columns + string rows, and `pg_type_for_exasol_data_type` for OID mapping. All of these are restored from git history (commit `db58cf5` and earlier).
- **Alternatives:** Build an Arrow shim that converts the WS `Vec<Vec<Option<String>>>` shape into `RecordBatch` on the WS boundary so `pg_server.rs` only knows Arrow — rejected because the user intent is explicit ("full pre-migration pipeline") and a shim would still leave the WS transport with subtly different OID semantics from the original.
- **Rationale:** The WS transport is the supported production path. Its result shape and OID mapping have years of real-world validation. Adopting Arrow's column metadata for the WS transport would re-litigate type-coercion decisions that the JSON `dataType` path already gets right.
- **Promotes to ADR:** yes

### [4] Unify `RowCount` to `i64`

- **Decision:** `ExasolOutcome::RowCount(i64)` is a single variant for both transports. The WS transport casts its native `usize` to `i64` at the transport boundary.
- **Alternatives:** Keep two variants (`RowCountUsize(usize)` and `RowCountI64(i64)`) — rejected because the PostgreSQL command-completion machinery consumes a single integer width and two variants force every consumer to match both.
- **Rationale:** Lowest surface area. The cast is a one-liner inside `WebSocketTransport::execute`.
- **Promotes to ADR:** no

### [5] `GatewayCursor` uses a single struct with an internal `CursorData` enum

- **Decision:** `GatewayCursor { data: CursorData, position, scroll, hold }` where `enum CursorData { Arrow { schema, batches }, Typed { columns, rows } }`. The cursor registry stays `HashMap<String, GatewayCursor>`.
- **Alternatives:**
    - Two concrete types (`ArrowCursor`, `TypedCursor`) behind a `trait Cursor` and `Box<dyn Cursor>` — rejected because cursor stepping (`position`, `scroll`, `hold`) is transport-agnostic and `dyn` adds heap allocation per cursor without buying anything.
    - Parallel registries (`HashMap<String, ArrowCursor>` + `HashMap<String, TypedCursor>`) — rejected because every cursor operation (`CLOSE`, `FETCH`, `MOVE`) would need to look in two maps.
- **Rationale:** Shared index logic, isolated variance only at the render path.
- **Promotes to ADR:** no

### [6] Bootstrap uses the configured transport

- **Decision:** `bootstrap.rs::run_interactive_bootstrap` opens its `ExasolSession` via `ExasolSession::connect(&config.exasol, ...)`, which now reads `config.transport`. Bootstrap exercises the same transport the gateway will use.
- **Alternatives:** Always use Arrow for bootstrap regardless of `exasol.transport` — rejected because it would let bootstrap silently mask a transport-specific config error (wrong fingerprint format, TLS mismatch) until the gateway starts serving traffic.
- **Rationale:** Bootstrap is the operator's first end-to-end check; it must verify the path that will actually run.
- **Promotes to ADR:** no

### [7] Parameterised integration-test harness in `tests/common/transport_matrix.rs`

- **Decision:** Add `tests/common/transport_matrix.rs` with a `for_each_transport(name, body)` helper. Every integration test parameterises over both transports through this helper.
- **Alternatives:**
    - Duplicate every integration test file under a `_websocket` suffix — rejected because it doubles the test surface for a single-axis variation.
    - Run integration tests under one transport only and trust manual testing for the other — rejected because the entire point of this plan is to keep both transports first-class.
- **Rationale:** One source of truth for the integration matrix; new tests opt in to both transports with three lines of boilerplate.
- **Promotes to ADR:** no

### [8] WebSocket transport is async-only (via `tokio-tungstenite`)

- **Decision:** Restore the deleted WS code, but port it onto `tokio-tungstenite` so `WebSocketTransport::execute` satisfies `async fn` directly. No `task::spawn_blocking` wrappers, no `block_in_place`. TLS uses `tokio-rustls`; `native-tls` is NOT re-introduced.
- **Alternatives:** Restore the original sync `tungstenite` code and wrap each call in `task::spawn_blocking` — rejected because the previous plan deliberately removed every `spawn_blocking` Exasol call site and that constraint is preserved in the read-only-query-path spec.
- **Rationale:** A single async-trait contract for both transports; no resurrection of the `std::sync::Mutex` / "guard cannot cross `.await`" constraints.
- **Promotes to ADR:** yes

### [9] No runtime fallback between transports

- **Decision:** Transport selection is purely configuration-driven and fixed for the lifetime of the process. There is no automatic fallback from Arrow to WebSocket (or vice versa) when a session fails at runtime.
- **Alternatives:** Detect transport-specific connection failures and retry on the other transport — rejected explicitly by user intent ("no automatic fallback at runtime").
- **Rationale:** Operators want a predictable, observable transport choice. Silent fallback would obscure which transport actually served a given session.
- **Promotes to ADR:** yes

## Review Findings

<!-- Populated by speq-implement after code review. -->
