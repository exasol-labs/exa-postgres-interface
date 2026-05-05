#!/usr/bin/env python3
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[1]
PREPROCESSOR = ROOT / "sql" / "exasol_sql_preprocessor.sql"


def load_preprocessor_namespace():
    text = PREPROCESSOR.read_text(encoding="utf-8")
    marker = "CREATE OR REPLACE PYTHON3 PREPROCESSOR SCRIPT PG_CATALOG.PG_SQL_PREPROCESSOR AS\n"
    start = text.index(marker) + len(marker)
    end = text.rindex("\n/")
    namespace = {}
    exec(text[start:end], namespace)
    return namespace


def test_rewrites_postgres_qualified_operator_syntax():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT a.attname
FROM PG_CATALOG.pg_attribute AS a
JOIN PG_CATALOG.pg_class AS c
  ON a.attrelid OPERATOR(PG_CATALOG.=) c.oid
WHERE a.attnum OPERATOR(PG_CATALOG.>) 0
"""
    translated = namespace["adapter_call"](sql)
    normalized = translated.upper()
    assert "OPERATOR(" not in translated.upper()
    assert "A.ATTRELID = C.OID" in normalized
    assert "A.ATTNUM > 0" in normalized


def test_rewrites_metabase_table_privileges_query():
    namespace = load_preprocessor_namespace()
    sql = """
with table_privileges as (
 select
   NULL as role,
   t.schemaname as schema,
   t.objectname as table,
   pg_catalog.has_any_column_privilege(current_user, '"' || replace(t.schemaname, '"', '""') || '"' || '.' || '"' || replace(t.objectname, '"', '""') || '"',  'update') as update,
   pg_catalog.has_any_column_privilege(current_user, '"' || replace(t.schemaname, '"', '""') || '"' || '.' || '"' || replace(t.objectname, '"', '""') || '"',  'select') as select,
   pg_catalog.has_any_column_privilege(current_user, '"' || replace(t.schemaname, '"', '""') || '"' || '.' || '"' || replace(t.objectname, '"', '""') || '"',  'insert') as insert,
   pg_catalog.has_table_privilege(     current_user, '"' || replace(t.schemaname, '"', '""') || '"' || '.' || '"' || replace(t.objectname, '"', '""') || '"',  'delete') as delete
 from (
   select schemaname, tablename as objectname from pg_catalog.pg_tables
   union
   select schemaname, viewname as objectname from pg_catalog.pg_views
   union
   select schemaname, matviewname as objectname from pg_catalog.pg_matviews
 ) t
 where t.schemaname !~ '^pg_'
   and t.schemaname <> 'information_schema'
   and pg_catalog.has_schema_privilege(current_user, t.schemaname, 'usage')
)
select t.*
from table_privileges t
"""
    translated = namespace["adapter_call"](sql)
    assert "HAS_ANY_COLUMN_PRIVILEGE" not in translated.upper()
    assert 'AS "select"' in translated
    assert "SYS.EXA_ALL_TABLES" in translated
    assert "SYS.EXA_ALL_VIEWS" in translated


def test_rewrites_metabase_describe_syncable_tables_query():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT
  "n"."nspname" AS "schema",
  "c"."relname" AS "name",
  CASE
    "c"."relkind"
    WHEN 'r' THEN 'TABLE'
    WHEN 'p' THEN 'PARTITIONED TABLE'
    WHEN 'v' THEN 'VIEW'
    WHEN 'f' THEN 'FOREIGN TABLE'
    WHEN 'm' THEN 'MATERIALIZED VIEW'
    ELSE NULL
  END AS "type",
  "d"."description" AS "description",
  NULLIF("stat"."n_live_tup", 0) AS "estimated_row_count"
FROM "pg_catalog"."pg_class" AS "c"
INNER JOIN "pg_catalog"."pg_namespace" AS "n"
  ON "c"."relnamespace" = "n"."oid"
LEFT JOIN "pg_catalog"."pg_description" AS "d"
  ON ("c"."oid" = "d"."objoid")
 AND ("d"."objsubid" = 0)
 AND ("d"."classoid" = 'pg_class'::regclass)
LEFT JOIN "pg_stat_user_tables" AS "stat"
  ON ("n"."nspname" = "stat"."schemaname")
 AND ("c"."relname" = "stat"."relname")
WHERE ("c"."relnamespace" = "n"."oid")
  AND ("n"."nspname" !~ '^pg_')
  AND ("n"."nspname" <> 'information_schema')
  AND c.relkind in ('r', 'p', 'v', 'f', 'm')
ORDER BY "type" ASC, "schema" ASC, "name" ASC
"""
    translated = namespace["adapter_call"](sql)
    normalized = translated.upper()
    assert '"pg_catalog"' not in translated
    assert '"pg_stat_user_tables"' not in translated
    assert "PG_CATALOG.PG_CLASS" in normalized
    assert "PG_CATALOG.PG_NAMESPACE" in normalized
    assert "PG_CATALOG.PG_DESCRIPTION" in normalized
    assert "PG_CATALOG.PG_STAT_USER_TABLES" in normalized
    assert "REGEXP_INSTR(n.nspname, '^pg_') = 0" in translated
    assert ".REGEXP_INSTR" not in normalized


def test_rewrites_metabase_describe_fields_query_family():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT c.column_name AS "name",
       col_description(CAST(CAST(format('%I.%I', CAST(c.table_schema AS text), CAST(c.table_name AS text)) AS regclass) AS oid), c.ordinal_position) AS "field-comment"
FROM information_schema.columns AS c
LEFT JOIN (
  SELECT tc.table_schema, tc.table_name, kc.column_name
  FROM information_schema.table_constraints AS tc
  JOIN information_schema.key_column_usage AS kc
    ON tc.constraint_name = kc.constraint_name
  WHERE tc.constraint_type = 'PRIMARY KEY'
) AS pk
  ON c.table_schema = pk.table_schema
WHERE c.table_schema !~ '^information_schema|catalog_history|pg_'
  AND c.table_schema IN ('NYC_UBER')
UNION ALL
SELECT pa.attname AS "name", NULL AS "field-comment"
FROM pg_catalog.pg_class AS pc
JOIN pg_catalog.pg_namespace AS pn ON pn.oid = pc.relnamespace
JOIN pg_catalog.pg_attribute AS pa ON pa.attrelid = pc.oid
JOIN pg_catalog.pg_type AS pt ON pt.oid = pa.atttypid
JOIN pg_catalog.pg_namespace AS ptn ON ptn.oid = pt.typnamespace
WHERE pc.relkind = 'm'
ORDER BY "table-schema", "table-name", "database-position"
"""
    translated = namespace["adapter_call"](sql)
    assert "COL_DESCRIPTION" not in translated.upper()
    assert "INFORMATION_SCHEMA.COLUMNS C" in translated
    assert '"database-type"' in translated
    assert "C.TABLE_SCHEMA IN ('NYC_UBER')" in translated


def test_rewrites_metabase_describe_fks_query_family():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT "fk_ns"."nspname" AS "fk-table-schema",
       "fk_table"."relname" AS "fk-table-name",
       "fk_column"."attname" AS "fk-column-name",
       "pk_ns"."nspname" AS "pk-table-schema",
       "pk_table"."relname" AS "pk-table-name",
       "pk_column"."attname" AS "pk-column-name"
FROM "pg_constraint" AS "c"
JOIN "pg_class" AS "fk_table" ON "c"."conrelid" = "fk_table"."oid"
JOIN "pg_namespace" AS "fk_ns" ON "c"."connamespace" = "fk_ns"."oid"
JOIN "pg_attribute" AS "fk_column" ON "c"."conrelid" = "fk_column"."attrelid"
JOIN "pg_class" AS "pk_table" ON "c"."confrelid" = "pk_table"."oid"
JOIN "pg_namespace" AS "pk_ns" ON "pk_table"."relnamespace" = "pk_ns"."oid"
JOIN "pg_attribute" AS "pk_column" ON "c"."confrelid" = "pk_column"."attrelid"
WHERE fk_ns.nspname !~ '^information_schema|catalog_history|pg_'
  AND "c"."contype" = 'f'::char
  AND "fk_column"."attnum" = ANY(c.conkey)
  AND "pk_column"."attnum" = ANY(c.confkey)
  AND "fk_ns"."nspname" IN ('NYC_UBER')
ORDER BY "fk-table-schema", "fk-table-name"
"""
    translated = namespace["adapter_call"](sql)
    assert "SYS.EXA_DBA_CONSTRAINT_COLUMNS" in translated
    assert "ANY(" not in translated
    assert "CC.CONSTRAINT_SCHEMA IN ('NYC_UBER')" in translated


def test_rewrites_metabase_primary_keys_query_family():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT result.TABLE_CAT AS "TABLE_CAT",
       result.TABLE_SCHEM AS "TABLE_SCHEM",
       result.TABLE_NAME AS "TABLE_NAME",
       result.COLUMN_NAME AS "COLUMN_NAME",
       result.KEY_SEQ AS "KEY_SEQ",
       result.PK_NAME AS "PK_NAME"
FROM (
  SELECT current_database() AS TABLE_CAT,
         n.nspname AS TABLE_SCHEM,
         ct.relname AS TABLE_NAME,
         a.attname AS COLUMN_NAME,
         (information_schema._pg_expandarray(i.indkey)).n AS KEY_SEQ,
         ci.relname AS PK_NAME,
         information_schema._pg_expandarray(i.indkey) AS KEYS,
         a.attnum AS A_ATTNUM,
         i.indnkeyatts as KEY_COUNT
  FROM pg_catalog.pg_class ct
  JOIN pg_catalog.pg_attribute a ON (ct.oid = a.attrelid)
  JOIN pg_catalog.pg_namespace n ON (ct.relnamespace = n.oid)
  JOIN pg_catalog.pg_index i ON (a.attrelid = i.indrelid)
  JOIN pg_catalog.pg_class ci ON (ci.oid = i.indexrelid)
  WHERE true
    AND n.nspname = 'DEMO_SHARED'
    AND ct.relname = 'DAILY_ORDER_KPI'
    AND i.indisprimary
) result
WHERE result.A_ATTNUM = (result.KEYS).x
  AND result.KEY_SEQ <= KEY_COUNT
ORDER BY result.table_name, result.pk_name, result.key_seq
"""
    translated = namespace["adapter_call"](sql)
    assert "SYS.EXA_DBA_CONSTRAINT_COLUMNS" in translated
    assert "_PG_EXPANDARRAY" not in translated.upper()
    assert "CC.CONSTRAINT_SCHEMA = 'DEMO_SHARED'" in translated
    assert "CC.CONSTRAINT_TABLE = 'DAILY_ORDER_KPI'" in translated


def test_rewrites_metabase_describe_indexes_query_family():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT tmp."table-schema", tmp."table-name",
       trim(BOTH '"' FROM pg_catalog.pg_get_indexdef(tmp.ci_oid, tmp.pos, false)) AS "field-name"
FROM (
  SELECT n.nspname AS "table-schema",
         ct.relname AS "table-name",
         ci.oid AS ci_oid,
         (information_schema._pg_expandarray(i.indkey)).n AS pos
  FROM pg_catalog.pg_class AS ct
  JOIN pg_catalog.pg_namespace AS n ON ct.relnamespace = n.oid
  JOIN pg_catalog.pg_index AS i ON ct.oid = i.indrelid
  JOIN pg_catalog.pg_class AS ci ON ci.oid = i.indexrelid
  WHERE pg_catalog.pg_get_expr(i.indpred, i.indrelid) IS NULL
) AS tmp
WHERE tmp.pos = 1
"""
    translated = namespace["adapter_call"](sql)
    assert '"field-name"' in translated
    assert "WHERE 1 = 0" in translated
    assert "_PG_EXPANDARRAY" not in translated.upper()


def test_rewrites_qlik_table_list_varchar_casts():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT * FROM (
  select CAST(current_database() AS VARCHAR(127)) AS "TABLE_CAT",
         CAST(nspname AS VARCHAR(127)) AS "TABLE_SCHEM",
         CAST(relname AS VARCHAR(127)) AS "TABLE_NAME",
         CAST(
           CASE
             WHEN nspname ~'^pg_temp' THEN 'LOCAL TEMPORARY'
             WHEN relkind = 'r' THEN CASE
               WHEN nspname IN ('pg_catalog', 'information_schema', 'pg_toast') THEN 'SYSTEM TABLE'
               ELSE 'TABLE'
             END
             WHEN relkind = 'v' THEN 'VIEW'
             WHEN relkind = 'm' THEN 'MATVIEW'
           END AS VARCHAR(127)
         ) AS "TABLE_TYPE",
         CAST('' AS VARCHAR(250)) AS "REMARKS"
  FROM pg_catalog.pg_class c, pg_catalog.pg_namespace n
  WHERE relkind IN('r', 'v', 'm') AND n.oid = relnamespace
) S
WHERE "TABLE_SCHEM" like 'DEMO_SALES'::varchar
  AND "TABLE_TYPE" IN ('TABLE'::varchar, 'SYSTEM TABLE'::varchar)
ORDER BY "TABLE_TYPE", "TABLE_CAT", "TABLE_SCHEM", "TABLE_NAME"
"""
    translated = namespace["adapter_call"](sql)
    assert "AS VARCHAR)" not in translated.upper()
    assert "CAST('DEMO_SALES' AS VARCHAR(2000000))" in translated
    assert "CAST('TABLE' AS VARCHAR(2000000))" in translated
    assert "CAST('SYSTEM TABLE' AS VARCHAR(2000000))" in translated


def test_rewrites_qlik_column_list_type_name_star_projection():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT
  table_catalog AS TABLE_CAT,
  table_schema AS TABLE_SCHEM,
  table_name AS TABLE_NAME,
  column_name AS COLUMN_NAME,
  type_name AS TYPE_NAME,
  ordinal_position AS ORDINAL_POSITION
FROM (
  SELECT
    CASE udt_name
      WHEN 'boolean' THEN 'bool'
      WHEN 'integer' THEN 'int4'
      WHEN 'character varying' THEN 'varchar'
      ELSE udt_name
    END AS type_name,
    *
  FROM INFORMATION_SCHEMA.COLUMNS
  WHERE table_schema LIKE 'DEMO_FINANCE'::varchar
    AND table_name LIKE 'OPEN_AR_V'::varchar
) S
ORDER BY table_catalog, table_schema, table_name, ordinal_position
"""
    translated = namespace["adapter_call"](sql)
    assert "TYPE_NAME, * FROM INFORMATION_SCHEMA.COLUMNS" not in translated.upper()
    assert 'TYPE_NAME, "TABLE_CATALOG", "TABLE_SCHEMA"' in translated.upper()
    assert "CAST('DEMO_FINANCE' AS VARCHAR(2000000))" in translated
    assert "CAST('OPEN_AR_V' AS VARCHAR(2000000))" in translated


def test_rewrites_dbvis_tablespace_size_functions():
    namespace = load_preprocessor_namespace()
    sql = """
SELECT
    ts.spcname "Tablespace",
    ro.rolname "Owner",
    pg_size_pretty(pg_tablespace_size(spcname)) "Size",
    pg_tablespace_location(ts.oid) "Location",
    ts.spcoptions "Options"
FROM
    pg_catalog.pg_tablespace ts,
    pg_catalog.pg_roles ro
WHERE
    ts.spcowner = ro.oid
"""
    translated = namespace["adapter_call"](sql)
    assert "PG_CATALOG.PG_SIZE_PRETTY" in translated
    assert "PG_CATALOG.PG_TABLESPACE_SIZE" in translated
    assert "PG_CATALOG.PG_TABLESPACE_LOCATION" in translated
    assert "PG_SIZE_PRETTY(" not in translated.replace("PG_CATALOG.PG_SIZE_PRETTY(", "")


if __name__ == "__main__":
    test_rewrites_postgres_qualified_operator_syntax()
    test_rewrites_metabase_table_privileges_query()
    test_rewrites_metabase_describe_syncable_tables_query()
    test_rewrites_metabase_describe_fields_query_family()
    test_rewrites_metabase_describe_fks_query_family()
    test_rewrites_metabase_primary_keys_query_family()
    test_rewrites_metabase_describe_indexes_query_family()
    test_rewrites_qlik_table_list_varchar_casts()
    test_rewrites_qlik_column_list_type_name_star_projection()
    test_rewrites_dbvis_tablespace_size_functions()
