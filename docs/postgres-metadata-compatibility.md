# PostgreSQL Metadata Compatibility

Metadata compatibility now lives primarily inside Exasol. The gateway installs
and activates an Exasol SQL preprocessor, and the database exposes two
PostgreSQL-shaped schemas:

* `PG_CATALOG`
* `INFORMATION_SCHEMA`

The gateway remains responsible for PostgreSQL protocol behavior, client
authentication flow, Exasol session setup, TLS policy, read-only routing, and a
small number of local PostgreSQL session primitives. Catalog objects and
metadata queries are expected to resolve inside Exasol.

Primary references:

* [Exasol metadata system tables](https://docs.exasol.com/db/latest/sql_references/system_tables/metadata_system_tables.htm)
* [PostgreSQL system catalogs](https://www.postgresql.org/docs/18/catalogs.html)
* [PostgreSQL information schema](https://www.postgresql.org/docs/18/information-schema.html)
* DbVisualizer PostgreSQL profile:
  `C:/Program Files/DbVisualizer/resources/profiles/postgresql8.xml`
* DBeaver PostgreSQL plugin source:
  [github.com/dbeaver/dbeaver/tree/devel/plugins](https://github.com/dbeaver/dbeaver/tree/devel/plugins)

## Installed Objects

`sql/postgres_catalog_compatibility.sql` creates:

* `PG_CATALOG`
* `INFORMATION_SCHEMA`
* PostgreSQL-shaped catalog views
* PostgreSQL-shaped information-schema views
* compatibility helper functions such as `FORMAT_TYPE`,
  `PG_GET_CONSTRAINTDEF`, `PG_GET_EXPR`, `PG_GET_INDEXDEF`,
  `PG_GET_VIEWDEF`, `PG_GET_USERBYID`, `OIDVECTORTYPES`, `TO_REGCLASS`,
  `PG_RELATION_SIZE`, and related helpers used by PostgreSQL clients

`sql/exasol_sql_preprocessor.sql` creates:

* `PG_DEMO.PG_SQL_PREPROCESSOR`

The gateway activates the preprocessor for each client session with:

```sql
ALTER SESSION SET SQL_PREPROCESSOR_SCRIPT = PG_DEMO.PG_SQL_PREPROCESSOR
```

## Surface Coverage

The current compatibility SQL is generated and maintained so that every
documented PostgreSQL 18 `pg_catalog` relation and `information_schema` view
exists in Exasol, and every documented column exists on the corresponding
compatibility object.

That means clients should not fail because a documented PostgreSQL catalog table
or column is missing. The semantic quality varies by object:

* Exasol-backed mappings return real metadata where Exasol has an equivalent.
* PostgreSQL-only features return empty relations or `NULL` placeholder columns.
* Some helper functions return best-effort textual definitions or compatibility
  values rather than true PostgreSQL internals.

## Core Mapping Matrix

| PostgreSQL surface | Current handling | Exasol source |
| --- | --- | --- |
| `pg_catalog.pg_database` | one logical catalog named `exasol` | synthetic |
| `pg_catalog.pg_namespace` | Exasol schemas plus compatibility schemas | `SYS.EXA_DBA_SCHEMAS` |
| `pg_catalog.pg_roles`, `pg_user`, `pg_authid`, `pg_auth_members` | users, roles, role grants | `SYS.EXA_DBA_USERS`, `SYS.EXA_DBA_ROLES`, `SYS.EXA_DBA_ROLE_PRIVS` |
| `pg_catalog.pg_class` | tables, views, and index-like entries | `SYS.EXA_DBA_TABLES`, `SYS.EXA_DBA_VIEWS`, `SYS.EXA_DBA_COLUMNS`, `SYS.EXA_DBA_INDICES` |
| `pg_catalog.pg_attribute` | columns | `SYS.EXA_DBA_COLUMNS` |
| `pg_catalog.pg_attrdef` | column defaults | `SYS.EXA_DBA_COLUMNS` |
| `pg_catalog.pg_description` | schema, object, and column comments | `SYS.EXA_DBA_SCHEMAS`, `SYS.EXA_DBA_OBJECTS`, `SYS.EXA_DBA_COLUMNS` |
| `pg_catalog.pg_constraint` | primary key, foreign key, not-null constraints | `SYS.EXA_DBA_CONSTRAINTS`, `SYS.EXA_DBA_CONSTRAINT_COLUMNS` |
| `pg_catalog.pg_index` | Exasol index/constraint-derived compatibility rows | `SYS.EXA_DBA_INDICES`, `SYS.EXA_DBA_CONSTRAINTS` |
| `pg_catalog.pg_proc` | Exasol functions and scripts as PostgreSQL routines | `SYS.EXA_DBA_FUNCTIONS`, `SYS.EXA_DBA_SCRIPTS` |
| `pg_catalog.pg_type` | compatibility type rows for common PostgreSQL type OIDs and mapped Exasol column types | synthetic plus Exasol column metadata |
| `pg_catalog.pg_tables`, `pg_views`, `pg_matviews` | tables/views; materialized views empty | `SYS.EXA_DBA_TABLES`, `SYS.EXA_DBA_VIEWS`, synthetic |
| `pg_catalog.pg_settings` | common PostgreSQL settings expected by clients | synthetic |
| `pg_catalog.pg_stat_activity` | session-style compatibility view | Exasol session metadata where practical |
| `pg_catalog.pg_locks` | empty compatibility view | synthetic |
| `pg_catalog.pg_foreign_server`, `pg_foreign_data_wrapper`, `pg_user_mappings` | empty compatibility views | synthetic |
| other documented `pg_catalog` relations | PostgreSQL-shaped placeholder views unless mapped above | synthetic |
| `information_schema.schemata` | schemas | `PG_CATALOG.PG_NAMESPACE` |
| `information_schema.tables`, `views`, `columns` | tables, views, columns | `SYS.EXA_DBA_TABLES`, `SYS.EXA_DBA_VIEWS`, `SYS.EXA_DBA_COLUMNS` |
| `information_schema.table_constraints`, `key_column_usage`, `referential_constraints` | Exasol constraints | `SYS.EXA_DBA_CONSTRAINTS`, `SYS.EXA_DBA_CONSTRAINT_COLUMNS` |
| `information_schema.constraint_column_usage`, `constraint_table_usage` | Exasol constraint usage | `SYS.EXA_DBA_CONSTRAINTS`, `SYS.EXA_DBA_CONSTRAINT_COLUMNS` |
| `information_schema.routines` | Exasol functions and scripts | `SYS.EXA_DBA_FUNCTIONS`, `SYS.EXA_DBA_SCRIPTS` |
| `information_schema.role_table_grants`, `table_privileges`, `role_routine_grants` | privilege metadata where available | Exasol privilege metadata |
| `information_schema.triggers` | empty compatibility view | synthetic |
| other documented `information_schema` views | PostgreSQL-shaped placeholder views unless mapped above | synthetic |

## SQL Preprocessor Responsibilities

The Exasol preprocessor does more than basic dialect conversion. It also
normalizes PostgreSQL catalog SQL that common clients emit.

Current categories:

* unqualified `pg_catalog` relation references;
* `information_schema` references;
* `current_database()`, `current_catalog`, and `current_schemas(true)[1]`;
* PostgreSQL casts such as `::regclass`;
* PostgreSQL regex operators `~` and `!~`;
* `ILIKE`;
* helper functions such as `format_type`, `pg_get_expr`,
  `pg_get_constraintdef`, `pg_get_viewdef`, `pg_get_userbyid`,
  `oidvectortypes`, `pg_relation_size`, `to_regclass`, and
  `has_schema_privilege`;
* known bad `sqlglot` output shapes that Exasol cannot parse directly;
* observed DbVisualizer and DBeaver metadata queries that use PostgreSQL-only
  constructs such as tuple joins, `ARRAY_AGG`, vector subscripts, and
  `LATERAL UNNEST`.

The preprocessor intentionally returns empty correctly-shaped metadata for
PostgreSQL features that Exasol does not support, such as triggers, foreign data
wrappers, policies, extensions, and many PostgreSQL-internal catalog families.

## Client Compatibility Notes

The compatibility layer has been exercised with:

* `psql`
* PostgreSQL JDBC extended query mode
* DbVisualizer schema browsing paths
* DBeaver PostgreSQL catalog browsing paths
* JDBC `DatabaseMetaData` smoke and compatibility probes

Recently fixed metadata patterns include:

* `FORMAT_TYPE(...)` and other PostgreSQL helper functions;
* `pg_get_constraintdef(oid, true)`;
* `pg_constraint` browser queries using `LATERAL UNNEST(c.conkey)`;
* information-schema column-detail queries using row-value tuple joins;
* trigger browser queries using `ARRAY_AGG`;
* type browser queries using PostgreSQL scalar subqueries over `pg_class`;
* foreign server metadata queries where Exasol treats `FS` as a problematic
  alias after quoting `PG_FOREIGN_SERVER`.

## Rules And Semantics

* The PostgreSQL database/catalog concept is flattened to one logical catalog:
  `exasol`.
* Exasol schemas are exposed as PostgreSQL schemas.
* Exasol tables and views are exposed through both `pg_catalog` and
  `information_schema` compatibility views.
* PostgreSQL type OIDs are compatibility values. They are not PostgreSQL storage
  internals.
* Unsupported object families should return an empty relation with the expected
  columns, not an object-not-found error.
* Unsupported columns on otherwise mapped objects should exist and return
  `NULL` or a stable compatibility value.
* The compatibility layer favors client stability over pretending Exasol has
  PostgreSQL-only engine features.

## Verification

Useful live checks:

```sql
SELECT COUNT(*)
FROM PG_CATALOG.PG_CLASS;

SELECT COUNT(*)
FROM PG_CATALOG.PG_ATTRIBUTE;

SELECT COUNT(*)
FROM INFORMATION_SCHEMA.COLUMNS;

SELECT PG_CATALOG.FORMAT_TYPE(23, NULL);

SELECT *
FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS
WHERE TABLE_SCHEMA = 'DEMO_FINANCE'
  AND TABLE_NAME = 'INVOICES';
```

For client-level verification, use:

```bash
scripts/run_jdbc_compatibility_suite.sh \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys \
  'EXASOL_PASSWORD'
```

## Known Limits

* Placeholder views prove relation/column existence, not full PostgreSQL
  behavior.
* PostgreSQL arrays, range types, enum internals, collations, extensions,
  publications/subscriptions, event triggers, policies, and foreign data wrapper
  internals are mostly placeholders.
* Constraint metadata is mapped for Exasol primary keys, foreign keys, and
  not-null constraints. PostgreSQL check/unique/exclusion semantics do not map
  cleanly because Exasol constraint support differs.
* Function definitions and object definitions are best-effort compatibility
  strings, not PostgreSQL `pg_get_*` exact output.
* New PostgreSQL clients may issue additional catalog SQL shapes that need
  preprocessor mappings even when the underlying catalog views already exist.
* **`reg*` OID casts are not generally translated (known issue).** Expressions
  like `<oid>::regnamespace::text`, `<oid>::regclass::text`, or
  `<oid>::regtype::text` transpile to `CAST(... AS REGNAMESPACE/REGCLASS/REGTYPE)`,
  which Exasol rejects (`syntax error, unexpected IDENTIFIER_PART_`) because it
  has no such types. Today these only work where a specific client query is
  intercepted in `rewrite_known_metadata_query` (e.g. Beekeeper Studio's table
  list). A general fix would rewrite the post-sqlglot `CAST(CAST(<expr> AS
  REGNAMESPACE) AS LONG VARCHAR)` form into a catalog name lookup such as
  `(SELECT NSPNAME FROM PG_CATALOG.PG_NAMESPACE WHERE OID = <expr>)` (and the
  PG_CLASS / PG_TYPE equivalents). The risk is that this produces a correlated
  scalar subquery, which Exasol rejects in some positions — see the `pg_type`
  composite-type workaround in `rewrite_sqlglot_edge_cases` — so it needs
  validation against a live Exasol before shipping.
