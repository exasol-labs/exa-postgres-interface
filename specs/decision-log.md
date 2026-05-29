# Architecture Decision Records

<!-- ADRs are numbered sequentially starting from ADR-001. Never renumber. -->
<!-- recorder-agent appends new ADRs from plan decision logs. -->

---

## ADR-001: Add a dedicated `StatementPlan::SetSearchPath` variant

**Date:** 2026-05-07
**Plan:** `add-set-search-path-support`
**Status:** Accepted

### Context

The gateway needed to translate `SET search_path = <schema>` into Exasol's `OPEN SCHEMA <schema>` and return a PostgreSQL `SET` command tag to the client. The existing `StatementPlan::Execute` variant carries whatever command tag is associated with the forwarded SQL, so reusing it would require a special-case override to suppress the `OPEN SCHEMA` tag. The handler would also need to mutate `SessionState.current_schema` — a side effect that Execute does not own — forcing session-state mutation logic into a generic path shared by all other executable statements.

### Decision

Introduce a new `StatementPlan::SetSearchPath { schema: String }` variant. The dispatcher in `pg_server.rs` matches on this variant exclusively to issue `OPEN SCHEMA "<schema>"` against Exasol, update `SessionState.current_schema`, and return the `SET` command tag.

### Options Considered

| Option | Verdict |
|--------|---------|
| Dedicated `SetSearchPath` variant | Chosen — keeps Execute's command-tag invariant clean; localizes the side-effect (mutating SessionState) to one handler arm |
| Reuse `Execute` after string-replacing SQL to `OPEN SCHEMA` | Rejected — forces a special-case command-tag override on a generic path and introduces session-state mutation into a code path shared by all executable statements |
| Translate to `OPEN SCHEMA` inside the translator layer | Rejected — moves the boundary; translator layer has no handle on SessionState and does not own session-level side effects |

### Consequences

Every `Execute` call site remains unchanged. The `SetSearchPath` arm is the single location where `SessionState.current_schema` is mutated on success. Adding future schema-state behaviors (e.g., schema stack) touches only this arm.

---

## ADR-002: Late-bind `SHOW search_path` value in the handler, not the classifier

**Date:** 2026-05-07
**Plan:** `add-set-search-path-support`
**Status:** Accepted

### Context

`classify_statement` in `policy.rs` is a pure function over SQL text with no session dependency. The existing `local_show()` helper returns static values keyed by parameter name (e.g., `"search_path" => "public"`). After `SET search_path` support was added, returning a static `"public"` would create a visible inconsistency between `SHOW search_path` and `SELECT current_schema()` once a schema has been opened.

### Decision

Add `StatementPlan::ShowSearchPath` (no payload). The handler in `pg_server.rs` reads `SessionState.current_schema` at execution time and returns the active schema, falling back to the documented default when none has been set. All other `SHOW <name>` keys retain static values in `policy.rs`.

### Options Considered

| Option | Verdict |
|--------|---------|
| `ShowSearchPath` variant resolved at handler time | Chosen — keeps classifier a pure function; confines mutex access to the handler that already locks SessionState |
| Thread `&SessionState` into `classify_statement` | Rejected — inverts the layer boundary; every call site would need to acquire the session mutex; complicates unit testing of the pure classifier |

### Consequences

The classifier remains unit-testable without any session setup. The handler is the single place that reads dynamic session state for `SHOW` responses. Future dynamic `SHOW` parameters follow the same pattern by adding a new zero-payload variant.

---

## ADR-003: Reject multi-schema `SET search_path` rather than silently truncating

**Date:** 2026-05-07
**Plan:** `add-set-search-path-support`
**Status:** Accepted

### Context

PostgreSQL clients frequently issue `SET search_path = schema1, schema2, ...` to establish a priority-ordered name-resolution chain. Exasol has no construct that maps to this semantics — it supports a single active schema at a time via `OPEN SCHEMA`. The gateway mission explicitly forbids silent emulation that hides material PostgreSQL/Exasol semantic differences.

### Decision

When `parse_search_path_value` detects a top-level comma in the right-hand side of `SET search_path`, classify the statement as `StatementPlan::Reject` with the message "only single-schema search paths are supported by the gateway".

### Options Considered

| Option | Verdict |
|--------|---------|
| Reject with a clear compatibility error | Chosen — makes the semantic boundary visible; consistent with the project mission |
| Silently keep the first schema and discard the rest | Rejected — hides the semantic mismatch; clients that rely on fallback resolution would receive wrong results silently |
| Silently no-op | Rejected — client believes the schema context was established when it was not |
| Attempt fallback resolution across schemas per query | Rejected — separate large design, out of scope for this plan |

### Consequences

Clients that issue multi-schema `SET search_path` receive a clear error and can adapt. The session remains usable for subsequent statements. Implementing multi-schema emulation in a future plan does not require changing this decision — it would introduce a new classifier branch before the rejection arm.

---

## ADR-004: Adopt upstream-source mining and synthetic pgJDBC coverage as complementary approaches

**Date:** 2026-05-08
**Plan:** `add-upstream-client-test-mining`
**Status:** Accepted

### Context

Real DBeaver and Metabase sessions surface gateway errors that the existing hand-curated probes miss. The team needed to decide how to systematically expand compatibility coverage: mine SQL from upstream open-source client source code, generate synthetic probes that exercise pgJDBC protocol surfaces (extended-query round-trips, parameter-status, cursors, SQLSTATE), capture live wire traffic, or run upstream test suites directly against the gateway. Live wire-traffic capture and running upstream suites were explicitly rejected by the user.

### Decision

Adopt both upstream-source mining (near-term primary approach) and synthetic pgJDBC protocol coverage (follow-up plan). Mining catches tool-specific SQL the team observes failing in real sessions. Synthetic coverage catches protocol-level surfaces that tool source code does not expose as plain SQL text.

### Options Considered

| Option | Verdict |
|--------|---------|
| Mining + synthetic pgJDBC, both adopted | Chosen — mining and synthetic address complementary coverage gaps; sequencing mining first lets the team validate feasibility before designing the synthetic layer |
| Mining only | Rejected — misses protocol-level surfaces (extended-query round-trips, parameter-status, prepared-statement describe, cursors, SQLSTATE) that no upstream SQL text can expose |
| Synthetic only | Rejected — misses the tool-specific SQL idioms that are the proximate cause of current field errors |
| Live wire-traffic capture | Rejected by user — operational complexity, privacy concerns, non-determinism |
| Run upstream test suites directly | Rejected by user — upstream suites assume a real PostgreSQL target; they would produce false failures unrelated to gateway compatibility |

### Consequences

The compatibility corpus grows along two independent axes. Mining follow-up plans (`add-dbeaver-mined-corpus`, `add-metabase-mined-corpus`) deliver full per-tool SQL coverage. A synthetic follow-up plan (`add-synthetic-pgjdbc-protocol-coverage`) addresses protocol-level surfaces. Both families plug into the same single-command harness entry point.

---

## ADR-005: Isolate Metabase-derived probes in an AGPL-3.0-declared directory

**Date:** 2026-05-08
**Plan:** `add-upstream-client-test-mining`
**Status:** Accepted

### Context

Metabase is distributed under AGPL-3.0. SQL strings extracted from Metabase source may carry the AGPL license. The team needed a policy that removes ambiguity about whether AGPL-derived text enters the gateway runtime artifact and avoids complicating the repository's overall license posture.

### Decision

Metabase-derived probes live exclusively under `tests/jdbc/upstream-mined/metabase/`. That directory contains a `README.md` and a `LICENSE-AGPL.txt` declaring AGPL-3.0 inheritance for that directory only. Build tooling MUST NOT bundle that directory into the gateway runtime artifact. The policy is reversible if a future legal review concludes the SQL strings are non-copyrightable facts under scenes-à-faire.

### Options Considered

| Option | Verdict |
|--------|---------|
| AGPL-isolated directory with explicit LICENSE notice | Chosen — conservative, removes build-artifact ambiguity, easy to relax later |
| Treat SQL fragments as non-copyrightable facts | Rejected — legal posture is unresolved; acting on this assumption without a formal review creates risk |
| Abandon Metabase mining entirely | Rejected — Metabase is a first-class target client; losing its SQL coverage is a material gap |
| Co-locate with Apache 2.0 test code | Rejected — conflates license regions; complicates future artifact audits |

### Consequences

The release artifact remains free of AGPL-derived content. Reviewers can audit Metabase probe provenance by inspecting one directory. Reversing isolation requires removing the LICENSE-AGPL.txt and moving the files — a single PR with no logic changes.

---

## ADR-006: DBeaver-derived probes co-located with attribution comments, no separate license file

**Date:** 2026-05-08
**Plan:** `add-upstream-client-test-mining`
**Status:** Accepted

### Context

DBeaver Community Edition is distributed under Apache 2.0, which is permissive and compatible with the repository's existing license posture. The question was whether to mirror Metabase's isolation for symmetry or handle DBeaver differently.

### Decision

DBeaver-derived probes live under `tests/jdbc/upstream-mined/dbeaver/` with attribution comments in each probe file (upstream project, source file path, commit SHA). No separate LICENSE file is needed for that directory.

### Options Considered

| Option | Verdict |
|--------|---------|
| Attribution comments only, no separate LICENSE | Chosen — Apache 2.0 is compatible; isolation adds friction without legal benefit |
| Mirror Metabase AGPL isolation for symmetry | Rejected — Apache 2.0 poses no license conflict; symmetry at the cost of friction is not a net gain |

### Consequences

DBeaver probes are reviewable alongside other Apache 2.0-compatible test code. Attribution comments trace each probe to its upstream source. No build-artifact exclusion rule is required for this directory.

---

## ADR-007: Per-tool baseline promotion via checked-in `tests/jdbc/baselines/<tool>.txt` files

**Date:** 2026-05-08
**Plan:** `add-upstream-client-test-mining`
**Status:** Accepted

### Context

The team wanted to pin per-tool regression baselines independently of the global must-pass set. An exploratory probe should be promotable to a tool-specific baseline once it has passed in CI, without elevating it to a global must-pass requirement. The mechanism needed to match the user's stated mental model ("pinned once it passes") with minimum machinery.

### Decision

Specify (but defer implementing) a per-tool baseline-promotion mechanism: `tests/jdbc/baselines/<tool>.txt` lists probe IDs that are pinned as regressions for that specific tool. The compatibility-suite reporter classifies outcomes as `must-pass-failure`, `tool-baseline-failure`, `exploratory-failure`, or `pass`. Implementation is delivered by the follow-up plan `add-per-tool-baseline-promotion`.

### Options Considered

| Option | Verdict |
|--------|---------|
| Checked-in `baselines/<tool>.txt` files | Chosen — matches user's mental model; flat file is easy to review in PRs; tool-baseline membership is orthogonal to the `Expectation` enum |
| Promote `EXPLORATORY` to `MUST_PASS` globally once a probe passes | Rejected — removes per-tool granularity; a probe that passes for DBeaver but not Metabase would incorrectly gate both |
| Annotation-based per-tool promotion in source | Rejected — couples baseline state to source changes; requires recompile to update a baseline |

### Consequences

Tool-specific regressions are visible in CI independently of the global must-pass gate. The baseline files are plain text and reviewable in pull requests. The `add-per-tool-baseline-promotion` follow-up plan has a concrete file-path contract and report-classification contract to implement against.

---

## ADR-008: Adopt exarrow-rs as the Exasol transport

**Date:** 2026-05-18
**Plan:** `change-exarrow-transport`
**Status:** Accepted

### Context

The gateway used a hand-rolled transport stack built on `tungstenite` + JSON + RSA login in `src/exasol.rs`. This bespoke implementation required maintaining ~400 lines of protocol code and four runtime dependencies (`tungstenite`, `native-tls`, `rsa`, `sha2`, plus `base64`). Exasol Labs publishes and maintains `exarrow-rs`, an async Tokio driver that returns Apache Arrow `RecordBatch` values directly and supports certificate fingerprint pinning natively via `rustls` + `aws-lc-rs`.

### Decision

Replace the hand-rolled transport in `src/exasol.rs` with `exarrow-rs = "0.12"`, using its native TCP transport (`features = ["native"]`).

### Options Considered

| Option | Verdict |
|--------|---------|
| Adopt `exarrow-rs` | Chosen — deletes ~400 lines of bespoke protocol code, removes four runtime dependencies, gains async Arrow results and native fingerprint pinning |
| Keep existing transport and retype results to `RecordBatch` | Rejected — leaves `rsa`, `native-tls`, and the manual protocol loop in place; gives up the maintenance win |
| Build a thin in-tree async WebSocket client on `tokio-tungstenite` plus custom Arrow decoder | Rejected — Exasol Labs already ships and maintains `exarrow-rs`; re-implementing it is gratuitous |

### Consequences

The bespoke protocol loop and its dependencies are deleted. Certificate fingerprint validation continues to work through `exarrow-rs` `ConnectionParams`. Future transport improvements track the `exarrow-rs` upstream release cadence rather than in-tree maintenance.

---

## ADR-009: Propagate Arrow RecordBatch through pg_server.rs

**Date:** 2026-05-18
**Plan:** `change-exarrow-transport`
**Status:** Accepted

### Context

The existing `GatewayResponse::TypedQuery` variant carried `rows: Vec<Vec<Option<String>>>` — pre-stringified values produced inside `src/exasol.rs`. With `exarrow-rs` returning `RecordBatch` values, the team had to decide how far Arrow propagation should go before converting to strings.

### Decision

Replace `GatewayResponse::TypedQuery { columns, rows: Vec<Vec<Option<String>>>, command_tag }` with `GatewayResponse::ArrowQuery { schema: arrow::datatypes::SchemaRef, batches: Vec<RecordBatch>, command_tag }`. Arrow values are rendered into pgwire's `DataRowEncoder` at the wire-protocol boundary. The cursor registry stores `Vec<RecordBatch>` instead of pre-stringified rows.

### Options Considered

| Option | Verdict |
|--------|---------|
| Full Arrow propagation to the wire boundary | Chosen — matches explicit user intent ("full propagation, no adapter shim"); gives one central Arrow-to-pgwire renderer to maintain |
| Convert Arrow batches to `Vec<Vec<Option<String>>>` inside `src/exasol.rs` | Rejected — forces a value-by-value `String` round-trip and loses the throughput benefit |

### Consequences

A single Arrow-to-pgwire renderer handles all result-returning paths. The cursor registry carries typed Arrow data rather than strings, enabling future columnar optimizations. The metadata builder paths that synthesise PostgreSQL catalog rows retain a text-row helper (`arrow_batches_to_text_rows`) as a scoped exception pending a separate metadata-builder rewrite.

---

## ADR-010: Use exarrow-rs built-in fingerprint and validation parameters

**Date:** 2026-05-18
**Plan:** `change-exarrow-transport`
**Status:** Accepted

### Context

The existing configuration supported `certificate_fingerprint`, `validate_certificate`, and a DSN-embedded fingerprint suffix with a defined precedence rule. After adopting `exarrow-rs`, the team needed to decide whether to translate these directly into `ConnectionParams` or to build a custom `rustls::ServerCertVerifier`.

### Decision

Translate `ExasolConfig.certificate_fingerprint`, `validate_certificate`, and the DSN-embedded fingerprint suffix directly into `exarrow_rs::ConnectionParams { certificate_fingerprint, validate_server_certificate, use_tls, ... }`. Preserve the existing precedence rule from `Endpoint::parse` (explicit config field wins over DSN suffix; `validate_certificate = false` with no fingerprint maps to `validate_server_certificate = false`).

### Options Considered

| Option | Verdict |
|--------|---------|
| Translate config fields directly into `ConnectionParams` | Chosen — lowest-friction way to preserve every existing configuration knob without forking driver internals |
| Build a custom `rustls::ServerCertVerifier` injected via an `exarrow-rs` extension hook | Rejected — `ConnectionParams` already accepts the fingerprint and validation flag; reimplementing the verifier locks into `exarrow-rs` internals |

### Consequences

All existing certificate validation configuration knobs continue to work without behavioral change. The precedence rule is preserved in one place (`Endpoint::parse`). No internal `exarrow-rs` APIs are forked.

---

## ADR-011: Replace std::sync::Mutex with tokio::sync::Mutex around the Exasol session

**Date:** 2026-05-18
**Plan:** `change-exarrow-transport`
**Status:** Accepted

### Context

`SessionState.exasol` previously used `std::sync::Mutex<ExasolSession>`. The new async `execute` call on `exarrow-rs` requires awaiting the driver while the session lock is held. A `std::sync::Mutex` guard cannot be held across `.await` points without risking deadlocks or requiring `block_in_place`.

### Decision

Change `SessionState.exasol` to `tokio::sync::Mutex<ExasolSession>`. The cursor registry, extended-query result cache, and current-schema cell also move to `tokio::sync::Mutex` for consistency and to eliminate lock-poisoning handling.

### Options Considered

| Option | Verdict |
|--------|---------|
| Use `tokio::sync::Mutex` throughout `SessionState` | Chosen — the canonical fix for holding a lock across `.await`; eliminates lock-poisoning handling |
| Keep `std::sync::Mutex` and wrap `exarrow-rs` calls in `task::spawn_blocking` | Rejected — explicitly excluded by the design interview ("no `block_in_place`"); adds unnecessary thread hops |

### Consequences

All session-state locking is async-safe. Lock-poisoning error handling is removed. Every call site that previously used `std::sync::MutexGuard` is updated to await the async lock. The bootstrap path drives async session calls through `handle.block_on(...)` to keep terminal I/O synchronous.
