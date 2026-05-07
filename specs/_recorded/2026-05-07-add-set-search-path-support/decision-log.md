# Decision Log: add-set-search-path-support

Date: 2026-05-07

## Interview

**Q:** How should the gateway handle multi-schema `SET search_path = schema1, schema2, ...`?
**A:** Reject with a clear PostgreSQL-compatible error explaining that only single-schema search paths are supported. Exasol cannot honor multiple simultaneously active schemas, and silent acceptance would hide the semantic mismatch from clients.

**Q:** How should `RESET search_path` and `SET search_path = DEFAULT` behave?
**A:** Treat them as no-op success responses. Exasol has no documented "close schema" command, so leaving the active schema state unchanged is the pragmatic choice; clients use these forms defensively and a hard error would break common workflows.

**Q:** Should `SHOW search_path` return a static `"public"` (the current behavior) or dynamically reflect the active schema?
**A:** Dynamic. The whole point of supporting `SET search_path` is to make schema state visible to clients, and `current_schema()` already reflects state from Exasol after `OPEN SCHEMA`. Returning a static value would create a visible inconsistency between `SHOW search_path` and `SELECT current_schema()`.

**Q:** Multi-token, quoted identifier extraction is fiddly — what input forms must the parser accept?
**A:** Three forms, derived from observed clients: double-quoted (`"DEMO_FINANCE"`, sent by the PostgreSQL JDBC driver / DBeaver), single-quoted (`'demo_finance'`, sent by some psql-style clients via `SET ... TO '...'`), and bare identifiers (`demo_finance`, used by `SET ... TO <ident>` in psql). The unquoted literal `DEFAULT` is a reserved sentinel and must be detected before identifier rules apply.

## Design Decisions

### [1] Add a dedicated `StatementPlan::SetSearchPath` variant

- **Decision:** Introduce a new `StatementPlan::SetSearchPath { schema: String }` variant rather than rewriting the SQL into `OPEN SCHEMA <schema>` and reusing `StatementPlan::Execute`.
- **Alternatives:** (a) Reuse `Execute` after string-replacing the SQL — keeps the variant set small; (b) translate to `OPEN SCHEMA` inside the translator layer instead of the classifier — moves the boundary.
- **Rationale:** `Execute` returns whatever command tag it carries to the client. PostgreSQL clients expect `SET` for `SET search_path`, not `OPEN SCHEMA`. Reusing `Execute` would force a special-case command-tag override on a generic path, and the handler would still need to mutate `SessionState.current_schema` — a side effect `Execute` does not own. A dedicated variant keeps every other Execute call site unchanged and makes the side-effect locus explicit.
- **Promotes to ADR:** yes

### [2] Late-bind `SHOW search_path` value in the handler, not the classifier

- **Decision:** Add `StatementPlan::ShowSearchPath` (no payload) and resolve the active schema in `pg_server.rs` from `SessionState.current_schema` at execution time. Other `SHOW <name>` keys keep their static values in `policy.rs`.
- **Alternatives:** Thread `&SessionState` into `classify_statement` so `local_show` can read the current schema directly.
- **Rationale:** `classify_statement` is currently a pure function over SQL text with no session dependency. Inverting that boundary forces every call site to acquire the session mutex and complicates testing. Late-binding only the dynamic key keeps the classifier pure and confines mutex access to the handler that already locks `SessionState`.
- **Promotes to ADR:** yes

### [3] Reject multi-schema `SET search_path` rather than silently truncating

- **Decision:** When `SET search_path` parses to more than one comma-separated schema, return `StatementPlan::Reject` with a compatibility-focused error message ("only single-schema search paths are supported by the gateway").
- **Alternatives:** (a) Silently keep the first schema and discard the rest; (b) silently no-op; (c) attempt fallback resolution by trying schemas in order on each query.
- **Rationale:** The mission explicitly forbids silent emulation that hides material PostgreSQL/Exasol differences. Exasol has no construct that maps to PostgreSQL's name-resolution-across-multiple-schemas semantics; pretending otherwise would create non-deterministic resolution. Option (c) is its own large design and is out of scope.
- **Promotes to ADR:** yes

### [4] `RESET search_path` and `SET search_path = DEFAULT` are no-ops

- **Decision:** Both forms return `ClientSet` and leave `SessionState.current_schema` unchanged.
- **Alternatives:** Reject as unsupported; track a sentinel "no schema open" state and surface it to `SHOW search_path` as the documented default.
- **Rationale:** Exasol has no "close schema" verb, so there is no Exasol-side action to perform. Clients (especially JDBC-based ones) issue these defensively at session boundaries; a hard rejection would break working flows. The decision pairs with the `SHOW search_path` default fallback so reset is observable through the existing default value.
- **Promotes to ADR:** no

### [5] Always wrap the schema name in double quotes when issuing `OPEN SCHEMA`

- **Decision:** The handler builds `OPEN SCHEMA "<schema>"` regardless of input quoting, doubling any embedded `"` to escape.
- **Alternatives:** Pass the literal identifier untouched; uppercase unquoted identifiers to match Exasol's default identifier folding.
- **Rationale:** DBeaver sends `SET search_path = "DEMO_FINANCE"` with quotes preserved, expecting the case to be honored. Always quoting preserves the case the client requested and avoids reserved-word collisions. Doubling embedded quotes is the standard Exasol identifier-escape rule.
- **Promotes to ADR:** no

### [6] Keep the parser regex-driven rather than introducing a SQL grammar

- **Decision:** Use a focused regex (`SET_SEARCH_PATH_RE`) plus a small `parse_search_path_value` helper for the right-hand side, instead of routing through a full SQL parser.
- **Alternatives:** Add a dependency on `polyglot-sql` or `sqlparser` to walk a real AST.
- **Rationale:** The other classifier code in `policy.rs` is regex-driven (e.g., `SET_ASSIGN_RE`, `RESET_RE`, `SHOW_RE`, `DECLARE_CURSOR_RE`). Matching that style keeps the diff small and avoids introducing a parser dependency for one new statement family. The right-hand side parsing is contained behind a helper so a future engine swap touches one function.
- **Promotes to ADR:** no

## Review Findings

<!-- Populated by speq-implement after code review. -->
