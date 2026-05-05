# Compatibility Matrix

This matrix summarizes the current PostgreSQL-to-Exasol compatibility posture.
It is intentionally conservative: “supported” means the gateway has an explicit
path for the behavior, not that PostgreSQL and Exasol semantics are identical.

| PostgreSQL area | Status | Exasol or gateway behavior |
| --- | --- | --- |
| Startup/authentication | Supported | Gateway accepts PostgreSQL startup and cleartext password auth, then authenticates to Exasol with the same credentials. |
| Simple Query protocol | Supported | Statements are split, classified, translated when needed, and executed against Exasol. |
| Extended Query protocol | Partial | Parse/bind/describe/execute flow is supported for text parameters and row-returning statements. Binary parameters are not implemented. |
| `SELECT` | Supported | Translated from PostgreSQL dialect to Exasol dialect in the gateway. |
| `INSERT` | Supported with caveats | Routed to Exasol `INSERT`; PostgreSQL-only syntax or semantics may be rejected or fail translation. |
| `UPDATE` | Supported with caveats | Routed to Exasol `UPDATE`; complex PostgreSQL `UPDATE ... FROM` semantics need additional live coverage. |
| `DELETE` | Supported with caveats | Routed to Exasol `DELETE`; Exasol remains the semantic source of truth. |
| `MERGE` | Supported with caveats | Routed to Exasol `MERGE` where syntax is compatible or translatable. |
| `TRUNCATE` | Supported with caveats | Routed to Exasol where equivalent behavior exists. |
| `CREATE TABLE`, `CREATE TABLE AS` | Selected support | Exasol-equivalent forms are supported; PostgreSQL-only storage, inheritance, partitioning, and identity shorthand require explicit translation or rejection. |
| `CREATE VIEW` | Selected support | Exasol-equivalent view definitions are supported after SQL translation. |
| `CREATE SCHEMA` | Supported | Routed to Exasol schema DDL. |
| `ALTER`, `DROP`, `COMMENT`, grants | Partial | Supported only where statement classification and Exasol-equivalent behavior are defined. |
| Transactions | Compatibility wrappers | `BEGIN`, `COMMIT`, and `ROLLBACK` are acknowledged for client compatibility. Savepoints and two-phase commit are unsupported. |
| SQL cursors | Gateway-managed partial | `DECLARE`, `FETCH`, `MOVE`, and `CLOSE` are supported for materialized read-only row sets. Binary, updatable, and positioned cursor operations are unsupported. |
| Session commands | Partial local handling | Common `SET`, `SHOW`, and driver capability probes are handled locally where needed for client compatibility. |
| Prepared statements | Partial | Extended Query protocol works for supported statements. SQL-level prepared statement commands and binary values are not complete. |
| `COPY` | Unsupported | Needs a separate design because Exasol uses different import/export mechanisms. |
| PostgreSQL maintenance commands | Unsupported | `VACUUM`, PostgreSQL `ANALYZE`, `REINDEX`, and similar commands have no direct gateway behavior. |
| Async notifications | Unsupported | `LISTEN`, `NOTIFY`, and `UNLISTEN` are PostgreSQL-specific. |
| PostgreSQL roles/users DDL | Unsupported by policy | Exasol has users/roles, but PostgreSQL role semantics are not mapped yet. |
| `pg_catalog` metadata | Broad compatibility | `PG_CATALOG` views/functions expose Exasol-backed metadata where possible and stable empty/`NULL` results for PostgreSQL-only fields. |
| `information_schema` metadata | Broad compatibility | `INFORMATION_SCHEMA` compatibility objects map common PostgreSQL client metadata requests to Exasol metadata. |
| PostgreSQL helper functions | Partial | Common catalog helper functions are implemented or stubbed in `PG_CATALOG`; unsupported helpers should fail clearly or return documented placeholders. |
| PostgreSQL-only engine objects | Unsupported no equivalent | Extensions, event triggers, rewrite rules, publications/subscriptions, text search objects, access methods, and many tablespace behaviors are not real Exasol features. |
| Client metadata edge cases | Ongoing compatibility work | Observed JDBC, DbVisualizer, DBeaver, Qlik, and Metabase metadata query shapes are tracked with regression tests as they are discovered. |

For metadata object details, see
[postgres-metadata-compatibility.md](postgres-metadata-compatibility.md).
For the compatibility test harness, see
[client-compatibility-test-framework.md](client-compatibility-test-framework.md).
