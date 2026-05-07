# Plan: Change PostgreSQL Read/Write Compatibility Scope

## Objective

Replace the earlier read-only prototype scope with a capability-driven PostgreSQL compatibility design. The gateway SHOULD support all valid PostgreSQL read and write operations where Exasol has an equivalent statement, function, metadata object, privilege model, transaction behavior, or a safe gateway-managed compatibility behavior.

The objective is not full PostgreSQL emulation. Exasol remains the executing database and source of truth. PostgreSQL syntax, protocol responses, metadata, and client workflow behavior are compatibility surfaces layered over Exasol behavior.

This plan is design and specification work first. Implementation MAY proceed only after the capability and translator ownership boundaries are clear.

The administration model is now a first-class goal. The default installation path SHOULD minimize required Exasol-side script installation and per-session preprocessor configuration. The preferred target architecture is gateway-owned PostgreSQL-to-Exasol translation in Rust, with Exasol-side compatibility SQL limited to stable metadata views/functions that cannot be owned cleanly by the gateway.

## Relevant Existing Specs

* `specs/mission.md`
* `specs/_plans/add-read-only-postgres-interface/plan.md`
* `specs/_plans/add-read-only-postgres-interface/protocol/read-only-query-path/spec.md`
* `specs/_plans/add-read-only-postgres-interface/sql/postgres-to-exasol-translation/spec.md`

The earlier plan remains useful as historical implementation context, but this plan supersedes its read-only capability policy.

## Research Sources

Primary sources used for the compatibility design:

* PostgreSQL 18 SQL command catalog: <https://www.postgresql.org/docs/18/sql-commands.html>
* PostgreSQL 18 cursor behavior: <https://www.postgresql.org/docs/18/sql-declare.html> and <https://www.postgresql.org/docs/18/sql-fetch.html>
* Exasol SQL reference: <https://docs.exasol.com/db/latest/sql_reference.htm>
* Exasol SQL standard compliance: <https://docs.exasol.com/db/latest/sql_references/sqlstandardcompliance.htm>
* Exasol DML and DDL statement pages for `INSERT`, `UPDATE`, `DELETE`, `MERGE`, `CREATE TABLE`, `DROP TABLE`, `COMMIT`, and `ROLLBACK`
* Exasol SQL preprocessor documentation: <https://docs.exasol.com/saas/database_concepts/sql_preprocessor.htm>
* Polyglot repository and package docs: <https://github.com/tobilg/polyglot>, <https://pypi.org/project/polyglot-sql/>, <https://polyglot.gh.tobilg.com/>
* SQLGlot documentation: <https://sqlglot.com/sqlglot.html>

Follow-up research as of 2026-05-05 confirms Polyglot is positioned as a Rust/WASM SQL transpilation library with PostgreSQL and Exasol dialect names. This makes it a plausible Rust gateway dependency, but the project MUST still prove PostgreSQL-to-Exasol fixture parity for the observed client queries before making it the default translator.

Spike result as of 2026-05-05: `translation/polyglot-sqlglot-spike.md` shows raw Polyglot is not a drop-in replacement for the current SQLGlot preprocessor because current compatibility depends on project-owned rewrites. Polyglot plus the current rewrite pipeline is viable enough to continue as the preferred gateway-layer candidate.

## Compatibility Model

PostgreSQL syntax SHALL be classified into one of these support outcomes:

* `direct-exasol`: PostgreSQL syntax can be translated to a materially equivalent Exasol statement or function.
* `exasol-with-caveats`: Exasol supports the statement family, but PostgreSQL semantics differ and the difference must be documented in command tags, errors, metadata, or user documentation.
* `gateway-managed`: Exasol has no direct equivalent, but the gateway can provide client-visible compatibility without pretending Exasol has native support. Cursors are the first expected example.
* `metadata-only`: PostgreSQL clients may query catalog or information-schema objects, but the object maps to Exasol metadata, empty compatibility views, or `NULL` columns.
* `unsupported-no-equivalent`: PostgreSQL syntax is valid in PostgreSQL but has no safe Exasol equivalent for this gateway.
* `unsupported-policy`: Exasol may have related behavior, but the gateway deliberately does not expose it yet because translation, safety, privileges, or protocol responses are not specified.

Supported behavior MUST be stated by capability family, not by a single read/write boolean.

## PostgreSQL To Exasol Syntax Coverage Map

The gateway SHOULD maintain a living compatibility matrix derived from the PostgreSQL command catalog and Exasol SQL reference. The first design pass groups commands as follows.

| PostgreSQL command family | Exasol equivalent or gateway design | Initial support direction |
| --- | --- | --- |
| `SELECT`, `WITH`, `VALUES`, set operations, joins, predicates, grouping, windows | Exasol query language, functions, casts, identifiers, and metadata rewrites | `direct-exasol` or `exasol-with-caveats` |
| `INSERT`, `INSERT ... SELECT`, `INSERT ... DEFAULT VALUES` | Exasol `INSERT`; Exasol supports constants, subquery inserts, multi-row values, defaults, and generated identity/default behavior | `direct-exasol` with translation tests |
| `UPDATE` | Exasol `UPDATE`; Exasol supports `FROM` and internally transforms some cases to `MERGE` | `exasol-with-caveats` |
| `DELETE`, `TRUNCATE` | Exasol `DELETE` and `TRUNCATE`; Exasol delete storage behavior differs because rows may be marked deleted before cleanup | `exasol-with-caveats` |
| `MERGE` | Exasol `MERGE`; syntax and matching restrictions differ, including Exasol `ON` equivalence-condition rules | `exasol-with-caveats` |
| `CREATE TABLE`, `CREATE TABLE AS`, `SELECT INTO` | Exasol `CREATE TABLE`, `CREATE TABLE AS`, `SELECT INTO TABLE`, type and constraint translation | `exasol-with-caveats` |
| `ALTER TABLE`, constraints, defaults, identity | Exasol `ALTER TABLE` variants for columns, constraints, distribution, partitioning | `exasol-with-caveats` |
| `DROP TABLE`, `DROP VIEW`, `DROP SCHEMA`, `DROP USER`, related drops | Exasol `DROP` statements where the object type exists | `exasol-with-caveats` |
| `CREATE VIEW`, `CREATE SCHEMA`, `COMMENT`, `GRANT`, `REVOKE`, role/user management | Exasol equivalent statements where object and privilege models align | `exasol-with-caveats` |
| `CREATE FUNCTION`, `CREATE PROCEDURE`, `CALL`, `DO`, procedural language commands | Exasol scripts, UDFs, stored procedures, and `CALL` where available; PostgreSQL PL/pgSQL is not equivalent | mostly `unsupported-no-equivalent` unless explicitly mapped |
| `COPY` | Exasol `IMPORT`/`EXPORT`, gateway protocol copy, or client-side streaming design | `unsupported-policy` until a bulk-load design exists |
| `BEGIN`, `COMMIT`, `ROLLBACK`, `END`, autocommit, transaction state | Exasol `COMMIT` and `ROLLBACK`; gateway transaction-status reporting must be explicit | `exasol-with-caveats` |
| `SAVEPOINT`, `ROLLBACK TO SAVEPOINT`, `RELEASE SAVEPOINT`, two-phase commit | No documented direct Exasol equivalent in the researched sources | `unsupported-no-equivalent` unless later proven otherwise |
| `DECLARE`, `FETCH`, `MOVE`, `CLOSE` SQL cursors | Gateway-managed cursor registry over Exasol query execution and cached/materialized result slices | `gateway-managed` |
| `PREPARE`, `EXECUTE`, `DEALLOCATE`; extended query Parse/Bind/Execute | PostgreSQL protocol/server prepared statements can be gateway-managed; Exasol-side prepared SQL equivalence needs proof | `gateway-managed` or `unsupported-policy` by parameter mode |
| `EXPLAIN`, `ANALYZE`, `VACUUM`, `REINDEX`, `CLUSTER`, `CHECKPOINT` | Exasol profiling/system statements may exist for some diagnostics, but PostgreSQL maintenance semantics do not map directly | mostly `unsupported-no-equivalent` |
| `LISTEN`, `NOTIFY`, subscriptions, publications, replication, logical decoding | No Exasol equivalent for PostgreSQL async notifications or replication objects | `unsupported-no-equivalent` |
| foreign data wrappers, table spaces, access methods, text search objects, policies, event triggers, rules, extensions | PostgreSQL-specific engine objects with no Exasol equivalent in the current gateway model | `unsupported-no-equivalent` |

The matrix MUST be expanded into concrete command-level entries before implementation. A command-level entry SHOULD include: PostgreSQL syntax, Exasol equivalent syntax or function, translation owner, protocol response behavior, metadata visibility, examples, tests, and unsupported cases.

## Cursor Design

PostgreSQL SQL cursors are valid syntax and are used by some clients. Exasol does not provide a native PostgreSQL-style SQL cursor object, so cursor support SHOULD be designed as gateway-managed compatibility.

Gateway-managed cursor design principles:

* `DECLARE cursor FOR <query>` SHOULD parse and validate that the cursor query is a supported row-returning query.
* The gateway SHOULD maintain a per-session cursor registry keyed by cursor name.
* Cursor rows MAY be materialized at `DECLARE` time or lazily at first `FETCH`, but the chosen behavior MUST be documented because it affects volatility, memory, transaction lifetime, and visibility of later writes.
* `FETCH` SHOULD return a PostgreSQL row-returning response with a `FETCH n` command tag.
* `MOVE` SHOULD adjust cursor position and return a PostgreSQL-compatible command tag without returning rows.
* `CLOSE` SHALL release cursor state.
* `WITH HOLD` and non-hold cursor lifetime MUST be tied to the gateway's transaction compatibility model.
* `SCROLL` support SHOULD require a materialized result set or a proven equivalent random-access result capability.
* `BINARY` cursor output SHOULD remain unsupported until binary result-format handling is specified.
* Updatable cursors, `FOR UPDATE`, `FOR SHARE`, `WHERE CURRENT OF`, and positioned `UPDATE`/`DELETE` SHOULD remain unsupported until row identity, locking, and Exasol write semantics are specified.
* Cursor memory, row-count, timeout, and spill-to-disk limits MUST be explicit configuration or documented server policy.

## Installation And Administration Direction

The current deployment requires:

* Installing a gateway binary and systemd configuration.
* Installing Exasol-side `PG_CATALOG` and `INFORMATION_SCHEMA` compatibility SQL.
* Earlier prototypes also installed `PG_DEMO.PG_SQL_PREPROCESSOR`.
* Ensuring each Exasol session activates the SQL preprocessor.
* Debugging translated SQL through Exasol preprocessor audit behavior.

This is too complex for a customer-facing installation path. The revised target SHOULD reduce required database administration to one stable compatibility SQL bundle, or eventually no database-side objects for clients that do not need PostgreSQL catalog compatibility beyond gateway-provided responses.

The preferred target architecture is:

* Gateway owns statement classification, PostgreSQL-to-Exasol dialect translation, client-specific metadata query rewrites, unsupported capability errors, and translation diagnostics.
* Exasol owns actual SQL execution and durable metadata.
* Exasol-side compatibility SQL owns stable PostgreSQL-shaped catalog views/functions only when a client query should execute normally against database metadata.
* Exasol-side SQL preprocessing becomes optional fallback, not the standard path; when present, the fallback object SHOULD be `PG_CATALOG.PG_SQL_PREPROCESSOR`, not `PG_DEMO`.
* The gateway binary SHOULD support an interactive first-run bootstrap that creates a config file, asks for one-time setup credentials without saving them, checks/installs compatibility schemas with permission, and prints systemd service guidance.

The migration SHOULD port existing edge-case logic from the legacy SQL preprocessor into a gateway translation pipeline:

1. Normalize PostgreSQL identifier and catalog-reference forms.
2. Match known client metadata query families before generic transpilation.
3. Transpile general SQL using `polyglot-sql` when fixture coverage is sufficient.
4. Apply project-owned Exasol edge-case rewrites that are not handled by Polyglot.
5. Reject unsafe or unsupported syntax before sending SQL to Exasol.
6. Log the original SQL, classification result, translation owner, rewritten SQL fingerprint, and failure stage.

## Translator Architecture Evaluation

The current implementation uses an Exasol-side Python preprocessor based on `sqlglot`. The broader read/write scope reopens this decision.

`sqlglot` strengths:

* Mature Python package with production/stable classifier on PyPI.
* Official PostgreSQL dialect support.
* Existing implementation already uses it in the Exasol-side Python preprocessor.
* Exasol is listed as a community dialect, so Exasol support exists but may receive lower priority than official dialects.
* Good AST customization, unsupported-error controls, and transformation APIs.

`sqlglot` concerns:

* Running translation inside Exasol couples dialect behavior to Exasol-side Python/preprocessor deployment.
* Some translations require schema/type context that is not always available by default.
* The Exasol dialect is community-supported, so project-owned fixes may be necessary for broad write support.

`polyglot-sql` strengths:

* Rust core, C FFI, Python bindings, and TypeScript/WASM bindings.
* Supports both PostgreSQL and Exasol dialect names.
* Native Rust integration could move translation into the application layer and avoid a Python preprocessor dependency for new behavior.
* The project reports broad SQLGlot fixture compatibility and includes guard rails for parser/formatter resource limits.

`polyglot-sql` concerns:

* PyPI classifier says beta, while SQLGlot is production/stable.
* It is newer and explicitly inspired by SQLGlot, so coverage and bug history for Exasol-specific write translation need project validation.
* Moving translation into the application layer changes where query text is rewritten, how Exasol audit entries look, and how session initialization fails.
* If translation runs in Rust before sending SQL to Exasol, metadata query rewrites currently living in the Exasol preprocessor must be ported, shared, or left in a hybrid setup.

Recommendation for the spec:

* Prefer gateway-owned translation for user experience and administration simplicity.
* Do not replace `sqlglot` immediately by specification alone.
* Add a translator abstraction to the gateway design with required behavior, diagnostics, and tests.
* Evaluate `polyglot-sql` as the default application-layer candidate using the command-level compatibility matrix and observed client-query fixtures.
* Treat the current Exasol-side `sqlglot` preprocessor as a migration source and fallback, not as the long-term default.
* Prefer a hybrid path only as a temporary migration stage with clear ownership boundaries.
* Require a fixture suite comparing current preprocessor output, Polyglot output, and direct Exasol execution before selecting a new default.

## Proposed Spec Deltas

* `protocol/read-write-statement-path/spec.md`
  * Replaces the read-only policy with capability-based statement routing, command completion, transaction status, and gateway-managed cursor behavior.
* `sql/postgres-to-exasol-compatibility/spec.md`
  * Defines the compatibility matrix and PostgreSQL-to-Exasol syntax/function mapping requirements.
* `translation/translator-engine-selection/spec.md`
  * Defines how SQLGlot, Polyglot, and hybrid translation options are evaluated before implementation.
* `translation/polyglot-sqlglot-spike.md`
  * Records the initial Polyglot versus SQLGlot parity spike and recommendation.
* `operations/gateway-owned-translation/spec.md`
  * Defines the simplified installation/admin model and the migration away from required Exasol-side SQL preprocessing.

## Design Tasks

1. Build the command-level PostgreSQL-to-Exasol compatibility matrix.
   * Start from PostgreSQL 18 SQL commands.
   * Map each command to Exasol syntax, gateway-managed behavior, metadata-only compatibility, or unsupported status.
   * Record links to PostgreSQL and Exasol docs for each supported or rejected family.

2. Define supported write capability phases.
   * Prefer capability slices such as DML, transaction commands, table DDL, schema/view DDL, privilege commands, and bulk operations.
   * Do not implement an all-or-nothing write switch.

3. Design the gateway-managed cursor layer.
   * Define cursor registry state.
   * Define memory and spill limits.
   * Define scrollability, holdability, transaction lifetime, and unsupported positioned writes.

4. Define PostgreSQL protocol response mapping for non-row-returning statements.
   * Command tags.
   * Affected-row counts.
   * ReadyForQuery transaction status.
   * Error recovery after failed write statements.

5. Evaluate translator engine options.
   * Run representative DQL, DML, DDL, transaction, and metadata fixtures through SQLGlot and Polyglot.
   * Compare produced SQL with Exasol parser/execution behavior.
   * Decide whether Polyglot plus gateway edge-case rewrites can become the default application-side translation path.
   * Define the migration path for removing `PG_DEMO.PG_SQL_PREPROCESSOR` from standard installation.

6. Design simplified installation and administration.
   * Define the minimum database objects required for client compatibility.
   * Define idempotent install/upgrade/uninstall flows.
   * Define a no-preprocessor session initialization path.
   * Define diagnostics that replace Exasol preprocessor audit visibility.

7. Define compatibility tests before implementation.
   * Tests SHOULD include direct Exasol control statements and gateway PostgreSQL statements.
   * Tests SHOULD include positive and negative cases for every supported statement family.
   * Tests MUST include unsupported PostgreSQL-only syntax returning clear errors.

## Verification Plan

Spec verification SHOULD include:

* Review the compatibility matrix for every PostgreSQL command family.
* Validate that every supported family names the Exasol equivalent or gateway-managed behavior.
* Validate that every unsupported family states `unsupported-no-equivalent` or `unsupported-policy`.
* Confirm cursor scenarios cover `DECLARE`, `FETCH`, `MOVE`, `CLOSE`, holdability, scrollability, and cleanup.
* Confirm translator selection scenarios require direct comparison against Exasol execution before any engine switch.
* Confirm installation scenarios distinguish required gateway config from optional database-side compatibility objects.
* Confirm the default path no longer requires installing or enabling an Exasol-side SQL preprocessor once gateway-owned translation is implemented.

No implementation tests are required for this design-only plan.

## Risks And Assumptions

* PostgreSQL has many valid commands whose object model is specific to PostgreSQL. The gateway must not present unsupported PostgreSQL objects as writable if Exasol has no equivalent.
* Exasol write support is real for core DML and DDL, but syntax, constraints, identity behavior, privilege behavior, transaction behavior, and command tags can differ materially from PostgreSQL.
* PostgreSQL clients may depend on cursors and prepared statements through both SQL commands and wire-protocol portals. These should be designed together.
* Broad write support raises safety expectations: transaction state, partial failure, autocommit, update counts, DDL commits, and privilege errors become client-visible.
* The SQL translation engine may become a product dependency. Maturity, release cadence, extensibility, error reporting, resource controls, and Exasol dialect fidelity matter more than raw syntax coverage claims.
* Moving translation into the gateway improves installation ergonomics but reduces Exasol audit visibility of original preprocessor transformations. Gateway logs and diagnostics must replace that operational signal.
* The current preprocessor contains valuable client-specific edge-case behavior discovered through DBVisualizer, DBeaver, Qlik, Metabase, and JDBC testing. Migration must port behavior deliberately, not discard it.

## Open Decisions

* Which PostgreSQL command families are the first write-capable implementation phase?
* Should the gateway expose table DDL before privilege and user-management DDL?
* Should `COPY` map to Exasol `IMPORT`/`EXPORT`, PostgreSQL wire COPY, or remain unsupported?
* Should transaction state be backed by Exasol autocommit control, local protocol compatibility, or both?
* Should cursors materialize all rows, stream rows with bounded buffering, or support both modes?
* What cursor result-size limit is acceptable for a systemd-managed gateway process?
* Can `polyglot-sql` become the default Rust dependency for application-layer translation?
* Which current Exasol-side metadata rewrites should become first-class gateway rewrite rules, and which should remain in Exasol compatibility views/functions?
* What database objects should remain mandatory after the SQL preprocessor is removed from the standard path?
