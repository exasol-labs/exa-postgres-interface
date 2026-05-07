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
