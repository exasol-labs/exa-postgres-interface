-- Optional SQL preprocessor fallback for PostgreSQL-to-Exasol translation.
--
-- Install this only in an Exasol environment whose Python script language
-- includes sqlglot. The exact SQL preprocessor callback contract can vary by
-- Exasol version and deployment; keep the activation SQL configurable in
-- config/example.toml until the target system is fixed.

CREATE SCHEMA IF NOT EXISTS PG_CATALOG;

ALTER SESSION SET SQL_PREPROCESSOR_SCRIPT = NULL;

CREATE OR REPLACE PYTHON3 PREPROCESSOR SCRIPT PG_CATALOG.PG_SQL_PREPROCESSOR AS
import re

ILIKE_RE = re.compile(
    r"([A-Za-z_][A-Za-z0-9_]*|\"[^\"]+\"|CAST\([^)]+\))\s+ILIKE\s+('(''|[^'])*'|[A-Za-z_][A-Za-z0-9_]*|\"[^\"]+\")",
    re.IGNORECASE,
)
CURRENT_DATABASE_RE = re.compile(r"(?i)\bcurrent_database\s*\(\s*\)")
CURRENT_CATALOG_RE = re.compile(r"(?i)\bcurrent_catalog(?:\s*\(\s*\))?")
CURRENT_SCHEMAS_FIRST_RE = re.compile(
    r"(?i)\(?\s*(?:pg_catalog\.)?current_schemas\s*\(\s*true\s*\)\s*\)?\s*\[\s*1\s*\]"
)
QUALIFIED_PG_CATALOG_RE = re.compile(r"(?i)\bpg_catalog\.")
QUALIFIED_INFO_SCHEMA_RE = re.compile(r"(?i)\binformation_schema\.")
OBJ_DESCRIPTION_RE = re.compile(
    r"(?is)(?:pg_catalog\.)?obj_description\s*\(\s*([^,]+?)\s*,\s*'(pg_namespace|pg_class)'\s*\)"
)
REGCLASS_LITERAL_RE = re.compile(
    r"(?i)'((?:pg_catalog\.)?[A-Za-z_][A-Za-z0-9_]*)'\s*::\s*regclass"
)
REGEX_NOT_MATCH_RE = re.compile(
    r"(?i)([A-Za-z_][A-Za-z0-9_.]*|\"[^\"]+\")\s*!~\s*('(?:''|[^'])*')"
)
REGEX_MATCH_RE = re.compile(
    r"(?i)([A-Za-z_][A-Za-z0-9_.]*|\"[^\"]+\")\s*~\s*('(?:''|[^'])*')"
)
PG_IDENTIFY_OBJECT_IDENTITY_RE = re.compile(
    r"(?is)\(\s*(?:pg_catalog\.)?pg_identify_object\s*\(\s*([^,]+?)\s*,\s*([^,]+?)\s*,\s*([^)]+?)\s*\)\s*\)\s*\.\s*identity\b"
)
PG_GET_VIEWDEF_PRETTY_RE = re.compile(
    r"(?is)(?:pg_catalog\.)?pg_get_viewdef\s*\(\s*([^,]+?)\s*,\s*(?:true|false)\s*\)"
)
PG_GET_EXPR_PRETTY_RE = re.compile(
    r"(?is)(?:pg_catalog\.)?pg_get_expr\s*\(\s*([^,]+?)\s*,\s*([^,]+?)\s*,\s*(?:true|false)\s*\)"
)
PG_GET_CONSTRAINTDEF_PRETTY_RE = re.compile(
    r"(?is)(?:pg_catalog\.)?pg_get_constraintdef\s*\(\s*([^,]+?)\s*,\s*(?:true|false)\s*\)"
)
SPECIAL_CATALOG_OBJECTS_RE = re.compile(
    r"(?i)\bPG_CATALOG\.(PG_FOREIGN_SERVER|PG_FOREIGN_DATA_WRAPPER)\b"
)
QUALIFIED_OPERATOR_RE = re.compile(
    r"(?i)\s+OPERATOR\s*\(\s*(?:PG_CATALOG|pg_catalog)\s*\.\s*(<>|!=|<=|>=|=|<|>)\s*\)\s*"
)
QUOTED_QUALIFIED_IDENTIFIER_RE = re.compile(
    r'"([A-Za-z_][A-Za-z0-9_]*)"\s*\.\s*"([A-Za-z_][A-Za-z0-9_]*)"'
)
RELATION_ALIAS_RE = re.compile(
    r'(?is)(\b(?:from|join|left(?:\s+outer)?\s+join|right(?:\s+outer)?\s+join|inner\s+join|full(?:\s+outer)?\s+join|cross\s+join)\s+'
    r'(?:[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)?|"[A-Za-z_][A-Za-z0-9_]*")'
    r'(?:\s+AS)?)\s+"([A-Za-z_][A-Za-z0-9_]*)"'
)

CATALOG_RELATIONS = [
    # GENERATED_CATALOG_RELATIONS_START
    "pg_aggregate",
    "pg_am",
    "pg_amop",
    "pg_amproc",
    "pg_available_extensions",
    "pg_attrdef",
    "pg_attribute",
    "pg_auth_members",
    "pg_authid",
    "pg_cast",
    "pg_class",
    "pg_collation",
    "pg_constraint",
    "pg_conversion",
    "pg_database",
    "pg_db_role_setting",
    "pg_default_acl",
    "pg_depend",
    "pg_description",
    "pg_enum",
    "pg_event_trigger",
    "pg_extension",
    "pg_foreign_data_wrapper",
    "pg_foreign_server",
    "pg_foreign_table",
    "pg_group",
    "pg_index",
    "pg_inherits",
    "pg_init_privs",
    "pg_language",
    "pg_largeobject",
    "pg_largeobject_metadata",
    "pg_locks",
    "pg_matviews",
    "pg_namespace",
    "pg_opclass",
    "pg_operator",
    "pg_opfamily",
    "pg_parameter_acl",
    "pg_partitioned_table",
    "pg_policy",
    "pg_proc",
    "pg_publication",
    "pg_publication_namespace",
    "pg_publication_rel",
    "pg_range",
    "pg_replication_origin",
    "pg_rewrite",
    "pg_roles",
    "pg_rules",
    "pg_sequences",
    "pg_seclabel",
    "pg_sequence",
    "pg_settings",
    "pg_shdepend",
    "pg_shdescription",
    "pg_shseclabel",
    "pg_stat_activity",
    "pg_stat_user_tables",
    "pg_statistic",
    "pg_statistic_ext",
    "pg_statistic_ext_data",
    "pg_subscription",
    "pg_subscription_rel",
    "pg_tables",
    "pg_tablespace",
    "pg_transform",
    "pg_trigger",
    "pg_ts_config",
    "pg_ts_config_map",
    "pg_ts_dict",
    "pg_ts_parser",
    "pg_ts_template",
    "pg_type",
    "pg_user",
    "pg_user_mapping",
    "pg_user_mappings",
    "pg_views",
    # GENERATED_CATALOG_RELATIONS_END
]

FUNCTION_REPLACEMENTS = {
    re.compile(r"(?i)(?<![\w.\"])\bformat_type\s*\("): "PG_CATALOG.FORMAT_TYPE(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_identify_object\s*\("): "PG_CATALOG.PG_IDENTIFY_OBJECT(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_functiondef\s*\("): "PG_CATALOG.PG_GET_FUNCTIONDEF(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_userbyid\s*\("): "PG_CATALOG.PG_GET_USERBYID(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_expr\s*\("): "PG_CATALOG.PG_GET_EXPR(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_constraintdef\s*\("): "PG_CATALOG.PG_GET_CONSTRAINTDEF(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_indexdef\s*\("): "PG_CATALOG.PG_GET_INDEXDEF(",
    re.compile(r"(?i)(?<![\w.\"])\boidvectortypes\s*\("): "PG_CATALOG.OIDVECTORTYPES(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_partkeydef\s*\("): "PG_CATALOG.PG_GET_PARTKEYDEF(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_ruledef\s*\("): "PG_CATALOG.PG_GET_RULEDEF(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_triggerdef\s*\("): "PG_CATALOG.PG_GET_TRIGGERDEF(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_get_viewdef\s*\("): "PG_CATALOG.PG_GET_VIEWDEF(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_encoding_to_char\s*\("): "PG_CATALOG.PG_ENCODING_TO_CHAR(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_total_relation_size\s*\("): "PG_CATALOG.PG_TOTAL_RELATION_SIZE(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_relation_size\s*\("): "PG_CATALOG.PG_RELATION_SIZE(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_size_pretty\s*\("): "PG_CATALOG.PG_SIZE_PRETTY(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_tablespace_size\s*\("): "PG_CATALOG.PG_TABLESPACE_SIZE(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_tablespace_location\s*\("): "PG_CATALOG.PG_TABLESPACE_LOCATION(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_stat_get_numscans\s*\("): "PG_CATALOG.PG_STAT_GET_NUMSCANS(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_stat_get_blocks_fetched\s*\("): "PG_CATALOG.PG_STAT_GET_BLOCKS_FETCHED(",
    re.compile(r"(?i)(?<![\w.\"])\bpg_stat_get_blocks_hit\s*\("): "PG_CATALOG.PG_STAT_GET_BLOCKS_HIT(",
    re.compile(r"(?i)(?<![\w.\"])\bto_regclass\s*\("): "PG_CATALOG.TO_REGCLASS(",
    re.compile(r"(?i)(?<![\w.\"])\bshobj_description\s*\("): "PG_CATALOG.SHOBJ_DESCRIPTION(",
    re.compile(r"(?i)(?<![\w.\"])\bcol_description\s*\("): "PG_CATALOG.COL_DESCRIPTION(",
    re.compile(r"(?i)(?<![\w.\"])\bhas_schema_privilege\s*\("): "PG_CATALOG.HAS_SCHEMA_PRIVILEGE(",
}
REGCLASS_OIDS = {
    "pg_class": "1259",
    "pg_database": "1262",
    "pg_namespace": "2615",
    "pg_proc": "1255",
    "pg_type": "1247",
}

JOIN_PREFIX_RE = re.compile(
    r'(?i)(\bfrom\s+|\bjoin\s+|\bleft(?:\s+outer)?\s+join\s+|\bright(?:\s+outer)?\s+join\s+|\binner\s+join\s+|\bfull(?:\s+outer)?\s+join\s+|\bcross\s+join\s+|,\s*)"?(%s)"?(?=\s|,|\)|$)'
    % "|".join(CATALOG_RELATIONS)
)

def normalize_ansi_quoted_postgres_identifiers(sql):
    sql = QUOTED_QUALIFIED_IDENTIFIER_RE.sub(
        lambda match: "{}.{}".format(match.group(1), match.group(2)),
        sql,
    )
    sql = RELATION_ALIAS_RE.sub(
        lambda match: "{} {}".format(match.group(1), match.group(2)),
        sql,
    )
    sql = JOIN_PREFIX_RE.sub(
        lambda match: "{}{}".format(match.group(1), match.group(2)),
        sql,
    )
    return sql

def extract_in_filter(sql, source_column, target_column):
    pattern = re.compile(
        r"(?is)\b{}\s+IN\s*\(([^)]*)\)".format(re.escape(source_column))
    )
    match = pattern.search(sql)
    if not match:
        return ""
    return " AND {} IN ({})".format(target_column, match.group(1).strip())

def extract_eq_filter(sql, source_column, target_column):
    pattern = re.compile(
        r"(?is)\b{}\s*=\s*('(?:''|[^'])*'|[A-Za-z_][A-Za-z0-9_.]*)".format(
            re.escape(source_column)
        )
    )
    match = pattern.search(sql)
    if not match:
        return ""
    return " AND {} = {}".format(target_column, match.group(1).strip())

def extract_in_or_eq_filter(sql, source_column, target_column):
    return extract_in_filter(sql, source_column, target_column) or extract_eq_filter(
        sql, source_column, target_column
    )

def rewrite_known_metadata_query(sql):
    sql = normalize_ansi_quoted_postgres_identifiers(sql)
    normalized = " ".join(sql.split()).lower()
    if (
        normalized.startswith("with table_privileges as (")
        and "has_any_column_privilege" in normalized
        and "has_table_privilege" in normalized
        and "from table_privileges" in normalized
    ):
        return """
SELECT
    CAST(NULL AS VARCHAR(128)) AS "role",
    object_schema AS "schema",
    object_name AS "table",
    TRUE AS "update",
    TRUE AS "select",
    TRUE AS "insert",
    TRUE AS "delete"
FROM (
    SELECT TABLE_SCHEMA AS object_schema, TABLE_NAME AS object_name
    FROM SYS.EXA_ALL_TABLES
    UNION
    SELECT VIEW_SCHEMA AS object_schema, VIEW_NAME AS object_name
    FROM SYS.EXA_ALL_VIEWS
) t
WHERE LOWER(object_schema) NOT LIKE 'pg\\_%'
  AND LOWER(object_schema) <> 'information_schema'
"""
    if (
        "from information_schema.columns" in normalized
        and "col_description" in normalized
        and "union all" in normalized
        and "from pg_catalog.pg_class" in normalized
    ):
        schema_filter = extract_in_filter(sql, "c.table_schema", "C.TABLE_SCHEMA")
        table_filter = extract_in_filter(sql, "c.table_name", "C.TABLE_NAME")
        return """
SELECT
    C.COLUMN_NAME AS "name",
    CASE
        WHEN COALESCE(C.UDT_SCHEMA, 'pg_catalog') IN ('public', 'pg_catalog') THEN C.UDT_NAME
        ELSE '"' || C.UDT_SCHEMA || '"."' || C.UDT_NAME || '"'
    END AS "database-type",
    C.ORDINAL_POSITION - 1 AS "database-position",
    C.TABLE_SCHEMA AS "table-schema",
    C.TABLE_NAME AS "table-name",
    CASE WHEN PK.COLUMN_NAME IS NULL THEN FALSE ELSE TRUE END AS "pk?",
    CAST(NULL AS VARCHAR(2000000)) AS "field-comment",
    CASE
        WHEN (C.COLUMN_DEFAULT IS NULL OR LOWER(C.COLUMN_DEFAULT) = 'null')
         AND C.IS_NULLABLE = 'NO'
         AND C.IS_IDENTITY = 'NO'
        THEN TRUE ELSE FALSE
    END AS "database-required",
    C.COLUMN_DEFAULT AS "database-default",
    CASE WHEN C.IS_IDENTITY <> 'NO' THEN TRUE ELSE FALSE END AS "database-is-auto-increment",
    CASE WHEN C.IS_GENERATED = 'ALWAYS' THEN TRUE ELSE FALSE END AS "database-is-generated",
    CASE WHEN C.IS_NULLABLE = 'YES' THEN TRUE ELSE FALSE END AS "database-is-nullable"
FROM INFORMATION_SCHEMA.COLUMNS C
LEFT JOIN (
    SELECT
        TC.TABLE_SCHEMA,
        TC.TABLE_NAME,
        KC.COLUMN_NAME
    FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS TC
    JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE KC
      ON TC.CONSTRAINT_NAME = KC.CONSTRAINT_NAME
     AND TC.TABLE_SCHEMA = KC.TABLE_SCHEMA
     AND TC.TABLE_NAME = KC.TABLE_NAME
    WHERE TC.CONSTRAINT_TYPE = 'PRIMARY KEY'
) PK
  ON C.TABLE_SCHEMA = PK.TABLE_SCHEMA
 AND C.TABLE_NAME = PK.TABLE_NAME
 AND C.COLUMN_NAME = PK.COLUMN_NAME
WHERE REGEXP_INSTR(C.TABLE_SCHEMA, '^information_schema|catalog_history|pg_') = 0
{schema_filter}
{table_filter}
ORDER BY "table-schema", "table-name", "database-position"
""".format(schema_filter=schema_filter, table_filter=table_filter)
    if (
        "from pg_constraint" in normalized
        and "fk-table-schema" in normalized
        and "pk-table-schema" in normalized
        and "any(c.conkey)" in normalized
    ):
        schema_filter = extract_in_filter(sql, "fk_ns.nspname", "CC.CONSTRAINT_SCHEMA")
        table_filter = extract_in_filter(sql, "fk_table.relname", "CC.CONSTRAINT_TABLE")
        return """
SELECT
    CC.CONSTRAINT_SCHEMA AS "fk-table-schema",
    CC.CONSTRAINT_TABLE AS "fk-table-name",
    CC.COLUMN_NAME AS "fk-column-name",
    CC.REFERENCED_SCHEMA AS "pk-table-schema",
    CC.REFERENCED_TABLE AS "pk-table-name",
    CC.REFERENCED_COLUMN AS "pk-column-name"
FROM SYS.EXA_DBA_CONSTRAINT_COLUMNS CC
JOIN SYS.EXA_DBA_CONSTRAINTS C
  ON C.CONSTRAINT_SCHEMA = CC.CONSTRAINT_SCHEMA
 AND C.CONSTRAINT_TABLE = CC.CONSTRAINT_TABLE
 AND C.CONSTRAINT_NAME = CC.CONSTRAINT_NAME
WHERE C.CONSTRAINT_TYPE = 'FOREIGN KEY'
  AND REGEXP_INSTR(CC.CONSTRAINT_SCHEMA, '^information_schema|catalog_history|pg_') = 0
{schema_filter}
{table_filter}
ORDER BY "fk-table-schema", "fk-table-name"
""".format(schema_filter=schema_filter, table_filter=table_filter)
    if (
        "information_schema._pg_expandarray" in normalized
        and "pg_index" in normalized
        and "i.indisprimary" in normalized
        and "key_seq" in normalized
        and "pk_name" in normalized
    ):
        schema_filter = extract_in_or_eq_filter(sql, "n.nspname", "CC.CONSTRAINT_SCHEMA")
        table_filter = extract_in_or_eq_filter(sql, "ct.relname", "CC.CONSTRAINT_TABLE")
        return """
SELECT
    'exasol' AS "TABLE_CAT",
    CC.CONSTRAINT_SCHEMA AS "TABLE_SCHEM",
    CC.CONSTRAINT_TABLE AS "TABLE_NAME",
    CC.COLUMN_NAME AS "COLUMN_NAME",
    CC.ORDINAL_POSITION AS "KEY_SEQ",
    CC.CONSTRAINT_NAME AS "PK_NAME"
FROM SYS.EXA_DBA_CONSTRAINT_COLUMNS CC
JOIN SYS.EXA_DBA_CONSTRAINTS C
  ON C.CONSTRAINT_SCHEMA = CC.CONSTRAINT_SCHEMA
 AND C.CONSTRAINT_TABLE = CC.CONSTRAINT_TABLE
 AND C.CONSTRAINT_NAME = CC.CONSTRAINT_NAME
WHERE C.CONSTRAINT_TYPE = 'PRIMARY KEY'
{schema_filter}
{table_filter}
ORDER BY CC.CONSTRAINT_TABLE, CC.CONSTRAINT_NAME, CC.ORDINAL_POSITION
""".format(schema_filter=schema_filter, table_filter=table_filter)
    if (
        "information_schema._pg_expandarray" in normalized
        and "pg_get_indexdef" in normalized
        and "pg_catalog.pg_index" in normalized
    ):
        return """
SELECT
    CAST(NULL AS VARCHAR(128)) AS "table-schema",
    CAST(NULL AS VARCHAR(128)) AS "table-name",
    CAST(NULL AS VARCHAR(128)) AS "field-name"
FROM (SELECT 1 AS DUMMY)
WHERE 1 = 0
"""
    if (
        "from pg_constraint" in normalized
        and "lateral unnest" in normalized
        and "array_agg(col.attname" in normalized
    ):
        schema_filter = "1 = 1"
        table_filter = "1 = 1"
        schema_match = re.search(r"(?is)\bsch\.nspname\s+LIKE\s+('(?:''|[^'])*')", sql)
        table_match = re.search(r"(?is)\btbl\.relname\s+LIKE\s+('(?:''|[^'])*')", sql)
        if schema_match:
            schema_filter = "C.CONSTRAINT_SCHEMA LIKE {}".format(schema_match.group(1))
        if table_match:
            table_filter = "C.CONSTRAINT_TABLE LIKE {}".format(table_match.group(1))
        return """
WITH CONSTRAINT_COLUMNS AS (
    SELECT
        CONSTRAINT_SCHEMA,
        CONSTRAINT_TABLE,
        CONSTRAINT_NAME,
        GROUP_CONCAT(COLUMN_NAME ORDER BY COALESCE(ORDINAL_POSITION, 0) SEPARATOR ',') AS COLUMNS,
        MAX(REFERENCED_SCHEMA) AS FOREIGN_SCHEMA_NAME,
        MAX(REFERENCED_TABLE) AS FOREIGN_TABLE_NAME,
        GROUP_CONCAT(REFERENCED_COLUMN ORDER BY COALESCE(ORDINAL_POSITION, 0) SEPARATOR ',') AS FOREIGN_COLUMNS
    FROM SYS.EXA_DBA_CONSTRAINT_COLUMNS
    GROUP BY CONSTRAINT_SCHEMA, CONSTRAINT_TABLE, CONSTRAINT_NAME
)
SELECT
    C.CONSTRAINT_NAME AS constraint_name,
    CASE
        WHEN C.CONSTRAINT_TYPE = 'PRIMARY KEY' THEN 'Primary Key'
        WHEN C.CONSTRAINT_TYPE = 'FOREIGN KEY' THEN 'Foreign Key'
        WHEN C.CONSTRAINT_TYPE = 'NOT NULL' THEN 'Check'
        ELSE C.CONSTRAINT_TYPE
    END AS constraint_type,
    C.CONSTRAINT_SCHEMA AS "schema_name",
    C.CONSTRAINT_TABLE AS "table_name",
    CC.COLUMNS AS "columns",
    CC.FOREIGN_SCHEMA_NAME AS "foreign_schema_name",
    CC.FOREIGN_TABLE_NAME AS "foreign_table_name",
    CC.FOREIGN_COLUMNS AS "foreign_columns",
    CASE
        WHEN C.CONSTRAINT_TYPE = 'PRIMARY KEY'
            THEN 'PRIMARY KEY (' || COALESCE(CC.COLUMNS, '') || ')'
        WHEN C.CONSTRAINT_TYPE = 'FOREIGN KEY'
            THEN 'FOREIGN KEY (' || COALESCE(CC.COLUMNS, '') || ') REFERENCES '
                 || COALESCE(CC.FOREIGN_SCHEMA_NAME, '') || '.'
                 || COALESCE(CC.FOREIGN_TABLE_NAME, '') || '('
                 || COALESCE(CC.FOREIGN_COLUMNS, '') || ')'
        WHEN C.CONSTRAINT_TYPE = 'NOT NULL'
            THEN COALESCE(CC.COLUMNS, '') || ' IS NOT NULL'
        ELSE C.CONSTRAINT_TYPE
    END AS definition
FROM SYS.EXA_DBA_CONSTRAINTS C
LEFT JOIN CONSTRAINT_COLUMNS CC
  ON CC.CONSTRAINT_SCHEMA = C.CONSTRAINT_SCHEMA
 AND CC.CONSTRAINT_TABLE = C.CONSTRAINT_TABLE
 AND CC.CONSTRAINT_NAME = C.CONSTRAINT_NAME
WHERE {schema_filter}
  AND {table_filter}
ORDER BY "schema_name", "table_name"
""".format(schema_filter=schema_filter, table_filter=table_filter)

    if (
        "from pg_catalog.pg_trigger" in normalized
        and "information_schema.triggers" in normalized
        and "array_agg(" in normalized
    ):
        return """
SELECT
    trigger_name AS "Trigger Name",
    trigger_catalog AS "Trigger Catalog",
    trigger_schema AS "Trigger Schema",
    CAST(NULL AS VARCHAR(2000000)) AS "Event Manipulation",
    action_orientation AS "Action Orientation",
    action_condition AS "Action Condition",
    action_statement AS "Action Statement",
    CAST(NULL AS VARCHAR(2000000)) AS "Procedure Name",
    CAST(NULL AS DECIMAL(18,0)) AS "proc_oid",
    action_timing AS "Condition Timing",
    event_object_catalog AS "Event Object Catalog",
    event_object_schema AS "Event Object Schema",
    event_object_table AS "Event Object Table",
    action_reference_old_table AS "Action ref Old Table",
    action_reference_new_table AS "Action ref New Table",
    CAST(NULL AS VARCHAR(32)) AS "Status"
FROM information_schema.triggers
WHERE 1 = 0
"""
    return sql


def rewrite_catalog_relations(sql):
    def repl(match):
        prefix = match.group(1)
        relation = match.group(2)
        upper_relation = relation.upper()
        if relation.lower() in {"pg_foreign_server", "pg_foreign_data_wrapper"}:
            return '{}PG_CATALOG."{}"'.format(prefix, upper_relation)
        return "{}PG_CATALOG.{}".format(prefix, upper_relation)

    return JOIN_PREFIX_RE.sub(repl, sql)


def rewrite_object_description(sql):
    def repl(match):
        obj_expr = match.group(1).strip()
        obj_type = match.group(2).lower()
        classoid = "2615" if obj_type == "pg_namespace" else "1259"
        return (
            "(SELECT D.DESCRIPTION FROM PG_CATALOG.PG_DESCRIPTION D "
            "WHERE D.OBJOID = {obj_expr} AND D.CLASSOID = {classoid} AND D.OBJSUBID = 0)"
        ).format(obj_expr=obj_expr, classoid=classoid)

    return OBJ_DESCRIPTION_RE.sub(repl, sql)


def rewrite_regex_operators(sql):
    sql = REGEX_NOT_MATCH_RE.sub(
        lambda match: "REGEXP_INSTR({}, {}) = 0".format(match.group(1), match.group(2)),
        sql,
    )
    sql = REGEX_MATCH_RE.sub(
        lambda match: "REGEXP_INSTR({}, {}) > 0".format(match.group(1), match.group(2)),
        sql,
    )
    return sql


def rewrite_regclass_literals(sql):
    def repl(match):
        name = match.group(1).split(".")[-1].lower()
        return REGCLASS_OIDS.get(name, "0")

    return REGCLASS_LITERAL_RE.sub(repl, sql)


def rewrite_qualified_operators(sql):
    return QUALIFIED_OPERATOR_RE.sub(lambda match: " {} ".format(match.group(1)), sql)


def rewrite_pg_catalog(sql):
    sql = normalize_ansi_quoted_postgres_identifiers(sql)
    sql = rewrite_known_metadata_query(sql)
    sql = rewrite_qualified_operators(sql)
    sql = rewrite_object_description(sql)
    sql = PG_IDENTIFY_OBJECT_IDENTITY_RE.sub(
        lambda match: "PG_CATALOG.PG_IDENTIFY_OBJECT({}, {}, {})".format(
            match.group(1).strip(),
            match.group(2).strip(),
            match.group(3).strip(),
        ),
        sql,
    )
    sql = PG_GET_VIEWDEF_PRETTY_RE.sub(
        lambda match: "PG_CATALOG.PG_GET_VIEWDEF({})".format(match.group(1).strip()),
        sql,
    )
    sql = PG_GET_EXPR_PRETTY_RE.sub(
        lambda match: "PG_CATALOG.PG_GET_EXPR({}, {})".format(
            match.group(1).strip(),
            match.group(2).strip(),
        ),
        sql,
    )
    sql = PG_GET_CONSTRAINTDEF_PRETTY_RE.sub(
        lambda match: "PG_CATALOG.PG_GET_CONSTRAINTDEF({})".format(match.group(1).strip()),
        sql,
    )
    sql = CURRENT_DATABASE_RE.sub("'exasol'", sql)
    sql = CURRENT_CATALOG_RE.sub("'exasol'", sql)
    sql = CURRENT_SCHEMAS_FIRST_RE.sub("'pg_catalog'", sql)
    sql = QUALIFIED_PG_CATALOG_RE.sub("PG_CATALOG.", sql)
    sql = QUALIFIED_INFO_SCHEMA_RE.sub("INFORMATION_SCHEMA.", sql)
    sql = rewrite_catalog_relations(sql)
    sql = SPECIAL_CATALOG_OBJECTS_RE.sub(
        lambda match: 'PG_CATALOG."{}"'.format(match.group(1).upper()),
        sql,
    )
    sql = rewrite_regex_operators(sql)
    sql = rewrite_regclass_literals(sql)
    for pattern, replacement in FUNCTION_REPLACEMENTS.items():
        sql = pattern.sub(replacement, sql)
    return sql

def rewrite_sqlglot_edge_cases(sql):
    information_schema_columns = (
        "TABLE_CATALOG", "TABLE_SCHEMA", "TABLE_NAME", "COLUMN_NAME",
        "ORDINAL_POSITION", "COLUMN_DEFAULT", "IS_NULLABLE", "DATA_TYPE",
        "CHARACTER_MAXIMUM_LENGTH", "CHARACTER_OCTET_LENGTH",
        "NUMERIC_PRECISION", "NUMERIC_PRECISION_RADIX", "NUMERIC_SCALE",
        "DATETIME_PRECISION", "INTERVAL_TYPE", "INTERVAL_PRECISION",
        "CHARACTER_SET_CATALOG", "CHARACTER_SET_SCHEMA", "CHARACTER_SET_NAME",
        "COLLATION_CATALOG", "COLLATION_SCHEMA", "COLLATION_NAME",
        "DOMAIN_CATALOG", "DOMAIN_SCHEMA", "DOMAIN_NAME", "UDT_CATALOG",
        "UDT_SCHEMA", "UDT_NAME", "SCOPE_CATALOG", "SCOPE_SCHEMA",
        "SCOPE_NAME", "MAXIMUM_CARDINALITY", "DTD_IDENTIFIER",
        "IS_SELF_REFERENCING", "IS_IDENTITY", "IDENTITY_GENERATION",
        "IDENTITY_START", "IDENTITY_INCREMENT", "IDENTITY_MAXIMUM",
        "IDENTITY_MINIMUM", "IDENTITY_CYCLE", "IS_GENERATED",
        "GENERATION_EXPRESSION", "IS_UPDATABLE"
    )
    information_schema_columns_projection = ", ".join(
        '"{}"'.format(column) for column in information_schema_columns
    )
    sql = sql.replace(
        'PG_CATALOG."PG_FOREIGN_SERVER" AS fs',
        'PG_CATALOG."PG_FOREIGN_SERVER" AS srv',
    )
    if 'PG_CATALOG."PG_FOREIGN_SERVER" AS srv' in sql:
        sql = re.sub(r"(?i)\bfs\.", "srv.", sql)
    sql = sql.replace(
        "ARRAY_AGG(CAST(event_manipulation AS LONG VARCHAR))",
        "LISTAGG(CAST(event_manipulation AS VARCHAR(2000000)), ', ') WITHIN GROUP (ORDER BY event_manipulation)",
    )
    sql = sql.replace(
        " WHERE p.prorettype <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype) AND (p.proargtypes[-1] IS NULL OR p.proargtypes[-1] <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype)) AND",
        " WHERE",
    )
    sql = sql.replace(
        " WHERE p.prorettype <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype) AND (p.proargtypes[-1] IS NULL OR p.proargtypes[-1] <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype))",
        " WHERE 1 = 1",
    )
    sql = sql.replace(
        "CASE p.proargtypes[-1] WHEN CAST('PG_CATALOG.\"any\"' AS PG_CATALOG.regtype) THEN CAST('(all types)' AS PG_CATALOG.text) ELSE PG_CATALOG.format_type(p.proargtypes[-1], NULL) END",
        "PG_CATALOG.OIDVECTORTYPES(p.proargtypes)",
    )
    sql = sql.replace(
        "(t.typrelid = 0 OR (SELECT c.relkind = 'c' FROM PG_CATALOG.pg_class AS c WHERE c.oid = t.typrelid))",
        "(t.typrelid = 0)",
    )
    sql = sql.replace(
        "ON (C.TABLE_CATALOG, C.TABLE_SCHEMA, C.TABLE_NAME, 'TABLE', C.DTD_IDENTIFIER) = (E.OBJECT_CATALOG, E.OBJECT_SCHEMA, E.OBJECT_NAME, E.OBJECT_TYPE, E.DTD_IDENTIFIER)",
        "ON C.TABLE_CATALOG = E.OBJECT_CATALOG AND C.TABLE_SCHEMA = E.OBJECT_SCHEMA AND C.TABLE_NAME = E.OBJECT_NAME AND E.OBJECT_TYPE = 'TABLE' AND C.DTD_IDENTIFIER = E.DTD_IDENTIFIER",
    )
    sql = sql.replace(
        "ON (C.TABLE_CATALOG, C.TABLE_SCHEMA, C.TABLE_NAME, C.COLUMN_NAME, 'column_name') = (CO.TABLE_CATALOG, CO.TABLE_SCHEMA, CO.TABLE_NAME, CO.COLUMN_NAME, CO.OPTION_NAME)",
        "ON C.TABLE_CATALOG = CO.TABLE_CATALOG AND C.TABLE_SCHEMA = CO.TABLE_SCHEMA AND C.TABLE_NAME = CO.TABLE_NAME AND C.COLUMN_NAME = CO.COLUMN_NAME AND CO.OPTION_NAME = 'column_name'",
    )
    sql = sql.replace(
        "CO.OPTION_VALUE AS COLUMN_OPTION, C.ORDINAL_POSITION, C.IS_IDENTITY",
        "CO.OPTION_VALUE AS COLUMN_OPTION, C.ORDINAL_POSITION AS ORDINAL_POSITION_DUP, C.IS_IDENTITY",
    )
    sql = re.sub(
        r"(?i)\bEND\s+AS\s+type_name\s*,\s*(?:[A-Z_][A-Z0-9_]*\.)?\*\s+FROM\s+INFORMATION_SCHEMA\.COLUMNS\b",
        "END AS type_name, {} FROM INFORMATION_SCHEMA.COLUMNS".format(
            information_schema_columns_projection
        ),
        sql,
    )
    sql = re.sub(r"(?i)\bAS\s+VARCHAR\s*\)", "AS VARCHAR(2000000))", sql)
    return sql

def rewrite_ilike(sql):
    return ILIKE_RE.sub(
        lambda match: "UPPER({}) LIKE UPPER({})".format(match.group(1), match.group(2)),
        sql,
    )

def adapter_call(sql_statement):
    try:
        normalized_statement = normalize_ansi_quoted_postgres_identifiers(sql_statement)
        known_metadata_query = rewrite_known_metadata_query(normalized_statement)
        if known_metadata_query != normalized_statement:
            return rewrite_ilike(known_metadata_query)
        import sqlglot
        rewritten = rewrite_pg_catalog(normalized_statement)
        translated = sqlglot.transpile(rewritten, read="postgres", write="exasol")[0]
        translated = rewrite_sqlglot_edge_cases(translated)
        return rewrite_ilike(translated)
    except Exception as exc:
        raise Exception("PostgreSQL-to-Exasol SQL translation failed: " + str(exc))
/
