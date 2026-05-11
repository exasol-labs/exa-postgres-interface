use regex::{Captures, Regex};
use std::sync::LazyLock;

#[derive(Debug, Clone, thiserror::Error)]
#[error("PostgreSQL-to-Exasol SQL translation failed: {message}")]
pub struct TranslationError {
    message: String,
}

impl TranslationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

static QUOTED_QUALIFIED_IDENTIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""([A-Za-z_][A-Za-z0-9_]*)"\s*\.\s*"([A-Za-z_][A-Za-z0-9_]*)""#).unwrap()
});
static RELATION_ALIAS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)(\b(?:from|join|left(?:\s+outer)?\s+join|right(?:\s+outer)?\s+join|inner\s+join|full(?:\s+outer)?\s+join|cross\s+join)\s+(?:[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)?|"[A-Za-z_][A-Za-z0-9_]*")(?:\s+AS)?)\s+"([A-Za-z_][A-Za-z0-9_]*)""#,
    )
    .unwrap()
});
static CATALOG_RELATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?i)(\bfrom\s+|\bjoin\s+|\bleft(?:\s+outer)?\s+join\s+|\bright(?:\s+outer)?\s+join\s+|\binner\s+join\s+|\bfull(?:\s+outer)?\s+join\s+|\bcross\s+join\s+|,\s*)"?(?P<rel>{})"?(\s|,|\)|$)"#,
        CATALOG_RELATIONS.join("|")
    ))
    .unwrap()
});
static REGEX_NOT_MATCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)([A-Za-z_][A-Za-z0-9_.]*|"[^"]+")\s*!~\s*('(?:''|[^'])*')"#).unwrap()
});
static REGEX_MATCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)([A-Za-z_][A-Za-z0-9_.]*|"[^"]+")\s*~\s*('(?:''|[^'])*')"#).unwrap()
});
static ILIKE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(CAST\([^)]+\)|'(?:''|[^'])*'|[A-Za-z_][A-Za-z0-9_]*|"[^"]+")\s+ILIKE\s+('(?:''|[^'])*'|[A-Za-z_][A-Za-z0-9_]*|"[^"]+")"#,
    )
    .unwrap()
});
static CURRENT_DATABASE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bcurrent_database\s*\(\s*\)").unwrap());
static CURRENT_CATALOG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bcurrent_catalog(?:\s*\(\s*\))?").unwrap());
static CURRENT_SCHEMA_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bcurrent_schema\s*\(\s*\)").unwrap());
static CURRENT_SCHEMAS_FIRST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\(?\s*(?:pg_catalog\.)?current_schemas\s*\(\s*true\s*\)\s*\)?\s*\[\s*1\s*\]")
        .unwrap()
});
static QUALIFIED_OPERATOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s+OPERATOR\s*\(\s*(?:PG_CATALOG|pg_catalog)\s*\.\s*(<>|!=|<=|>=|=|<|>)\s*\)\s*",
    )
    .unwrap()
});
static PG_IDENTIFY_OBJECT_IDENTITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)\(\s*(?:pg_catalog\.)?pg_identify_object\s*\(\s*([^,]+?)\s*,\s*([^,]+?)\s*,\s*([^)]+?)\s*\)\s*\)\s*\.\s*identity\b").unwrap()
});
static PG_GET_VIEWDEF_PRETTY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)(?:pg_catalog\.)?pg_get_viewdef\s*\(\s*([^,]+?)\s*,\s*(?:true|false)\s*\)")
        .unwrap()
});
static PG_GET_EXPR_PRETTY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)(?:pg_catalog\.)?pg_get_expr\s*\(\s*([^,]+?)\s*,\s*([^,]+?)\s*,\s*(?:true|false)\s*\)").unwrap()
});
static PG_GET_CONSTRAINTDEF_PRETTY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)(?:pg_catalog\.)?pg_get_constraintdef\s*\(\s*([^,]+?)\s*,\s*(?:true|false)\s*\)",
    )
    .unwrap()
});
static OBJ_DESCRIPTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)(?:pg_catalog\.)?obj_description\s*\(\s*([^,]+?)\s*,\s*'(pg_namespace|pg_class)'\s*\)").unwrap()
});
static REGCLASS_LITERAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)'((?:pg_catalog\.)?[A-Za-z_][A-Za-z0-9_]*)'\s*::\s*regclass").unwrap()
});
static QUOTE_IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bquote_ident\s*\(\s*([^()]+?)\s*\)").unwrap());

const CATALOG_RELATIONS: &[&str] = &[
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
];

const FUNCTION_REPLACEMENTS: &[(&str, &str)] = &[
    ("format_type", "PG_CATALOG.FORMAT_TYPE"),
    ("pg_identify_object", "PG_CATALOG.PG_IDENTIFY_OBJECT"),
    ("pg_get_functiondef", "PG_CATALOG.PG_GET_FUNCTIONDEF"),
    ("pg_get_userbyid", "PG_CATALOG.PG_GET_USERBYID"),
    ("pg_get_expr", "PG_CATALOG.PG_GET_EXPR"),
    ("pg_get_constraintdef", "PG_CATALOG.PG_GET_CONSTRAINTDEF"),
    ("pg_get_indexdef", "PG_CATALOG.PG_GET_INDEXDEF"),
    ("oidvectortypes", "PG_CATALOG.OIDVECTORTYPES"),
    ("pg_get_partkeydef", "PG_CATALOG.PG_GET_PARTKEYDEF"),
    ("pg_get_ruledef", "PG_CATALOG.PG_GET_RULEDEF"),
    ("pg_get_triggerdef", "PG_CATALOG.PG_GET_TRIGGERDEF"),
    ("pg_get_viewdef", "PG_CATALOG.PG_GET_VIEWDEF"),
    ("pg_encoding_to_char", "PG_CATALOG.PG_ENCODING_TO_CHAR"),
    (
        "pg_total_relation_size",
        "PG_CATALOG.PG_TOTAL_RELATION_SIZE",
    ),
    ("pg_relation_size", "PG_CATALOG.PG_RELATION_SIZE"),
    ("pg_size_pretty", "PG_CATALOG.PG_SIZE_PRETTY"),
    ("pg_tablespace_size", "PG_CATALOG.PG_TABLESPACE_SIZE"),
    (
        "pg_tablespace_location",
        "PG_CATALOG.PG_TABLESPACE_LOCATION",
    ),
    ("pg_stat_get_numscans", "PG_CATALOG.PG_STAT_GET_NUMSCANS"),
    (
        "pg_stat_get_blocks_fetched",
        "PG_CATALOG.PG_STAT_GET_BLOCKS_FETCHED",
    ),
    (
        "pg_stat_get_blocks_hit",
        "PG_CATALOG.PG_STAT_GET_BLOCKS_HIT",
    ),
    ("to_regclass", "PG_CATALOG.TO_REGCLASS"),
    ("shobj_description", "PG_CATALOG.SHOBJ_DESCRIPTION"),
    ("col_description", "PG_CATALOG.COL_DESCRIPTION"),
    ("has_schema_privilege", "PG_CATALOG.HAS_SCHEMA_PRIVILEGE"),
];

const INFORMATION_SCHEMA_COLUMNS: &[&str] = &[
    "TABLE_CATALOG",
    "TABLE_SCHEMA",
    "TABLE_NAME",
    "COLUMN_NAME",
    "ORDINAL_POSITION",
    "COLUMN_DEFAULT",
    "IS_NULLABLE",
    "DATA_TYPE",
    "CHARACTER_MAXIMUM_LENGTH",
    "CHARACTER_OCTET_LENGTH",
    "NUMERIC_PRECISION",
    "NUMERIC_PRECISION_RADIX",
    "NUMERIC_SCALE",
    "DATETIME_PRECISION",
    "INTERVAL_TYPE",
    "INTERVAL_PRECISION",
    "CHARACTER_SET_CATALOG",
    "CHARACTER_SET_SCHEMA",
    "CHARACTER_SET_NAME",
    "COLLATION_CATALOG",
    "COLLATION_SCHEMA",
    "COLLATION_NAME",
    "DOMAIN_CATALOG",
    "DOMAIN_SCHEMA",
    "DOMAIN_NAME",
    "UDT_CATALOG",
    "UDT_SCHEMA",
    "UDT_NAME",
    "SCOPE_CATALOG",
    "SCOPE_SCHEMA",
    "SCOPE_NAME",
    "MAXIMUM_CARDINALITY",
    "DTD_IDENTIFIER",
    "IS_SELF_REFERENCING",
    "IS_IDENTITY",
    "IDENTITY_GENERATION",
    "IDENTITY_START",
    "IDENTITY_INCREMENT",
    "IDENTITY_MAXIMUM",
    "IDENTITY_MINIMUM",
    "IDENTITY_CYCLE",
    "IS_GENERATED",
    "GENERATION_EXPRESSION",
    "IS_UPDATABLE",
];

pub fn translate_postgres_to_exasol(sql: &str) -> Result<String, TranslationError> {
    if is_exasol_passthrough_sql(sql) {
        return Ok(sql.to_owned());
    }

    let normalized = normalize_ansi_quoted_postgres_identifiers(sql);
    let known_metadata_query = rewrite_known_metadata_query(&normalized);
    if known_metadata_query != normalized {
        return Ok(rewrite_ilike(&known_metadata_query));
    }

    let rewritten = rewrite_pg_catalog(&normalized);
    let translated = polyglot_sql::transpile_by_name(&rewritten, "postgres", "exasol")
        .map_err(|err| TranslationError::new(err.to_string()))?
        .join("; ");
    let translated = rewrite_sqlglot_edge_cases(&translated);
    Ok(rewrite_ilike(&translated))
}

fn is_exasol_passthrough_sql(sql: &str) -> bool {
    let detection_sql = strip_sql_comments_for_detection(sql);
    let normalized = normalize_for_match(&detection_sql);
    if normalized.starts_with("with ") {
        return false;
    }

    let padded = format!(" {normalized} ");
    let postgres_specific_tokens = [
        "::",
        " ilike ",
        " operator(",
        " current_database(",
        " current_catalog",
        " current_schemas(",
        " current_schema(",
        " obj_description(",
        " shobj_description(",
        " col_description(",
        " format_type(",
        " oidvectortypes(",
        " to_regclass(",
        " unnest(",
        " generate_series(",
        " returning ",
        " any(",
        " array[",
        " regexp_matches(",
        " regexp_replace(",
        " quote_ident(",
    ];

    if postgres_specific_tokens
        .iter()
        .any(|token| padded.contains(token))
    {
        return false;
    }

    if padded.contains("pg_catalog.")
        || padded.contains("information_schema.")
        || CATALOG_RELATIONS
            .iter()
            .any(|rel| contains_word(&padded, rel))
    {
        return false;
    }

    if REGEX_MATCH_RE.is_match(sql) || REGEX_NOT_MATCH_RE.is_match(sql) {
        return false;
    }

    true
}

fn strip_sql_comments_for_detection(sql: &str) -> String {
    let mut output = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(ch) = chars.next() {
        if in_single_quote {
            output.push(ch);
            if ch == '\'' {
                if chars.peek() == Some(&'\'') {
                    output.push(chars.next().unwrap());
                } else {
                    in_single_quote = false;
                }
            }
            continue;
        }

        if in_double_quote {
            output.push(ch);
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    output.push(chars.next().unwrap());
                } else {
                    in_double_quote = false;
                }
            }
            continue;
        }

        if ch == '\'' {
            in_single_quote = true;
            output.push(ch);
            continue;
        }

        if ch == '"' {
            in_double_quote = true;
            output.push(ch);
            continue;
        }

        if ch == '-' && chars.peek() == Some(&'-') {
            chars.next();
            output.push(' ');
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            output.push(' ');
            let mut previous = '\0';
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    output.push('\n');
                }
                if previous == '*' && comment_ch == '/' {
                    break;
                }
                previous = comment_ch;
            }
            continue;
        }

        output.push(ch);
    }

    output
}

fn normalize_ansi_quoted_postgres_identifiers(sql: &str) -> String {
    let sql = QUOTED_QUALIFIED_IDENTIFIER_RE
        .replace_all(sql, "$1.$2")
        .to_string();
    let sql = RELATION_ALIAS_RE.replace_all(&sql, "$1 $2").to_string();
    CATALOG_RELATION_RE
        .replace_all(&sql, |cap: &Captures| {
            format!(
                "{}{}{}",
                cap.get(1).map_or("", |m| m.as_str()),
                cap.name("rel").map_or("", |m| m.as_str()),
                cap.get(3).map_or("", |m| m.as_str())
            )
        })
        .to_string()
}

fn rewrite_known_metadata_query(sql: &str) -> String {
    let normalized = normalize_for_match(sql);
    let compact = normalized.replace(' ', "");

    if normalized == "select collation_schema, collation_name from information_schema.collations" {
        return "SELECT \"COLLATION_SCHEMA\", \"COLLATION_NAME\" FROM INFORMATION_SCHEMA.\"COLLATIONS\"".to_owned();
    }

    if compact == "selectl.oid,l.*frompg_catalog.pg_foreign_serverl" {
        return concat!(
            "SELECT l.oid, l.srvname, l.srvowner, l.srvfdw, l.srvtype, ",
            "l.srvversion, l.srvacl, l.srvoptions ",
            "FROM PG_CATALOG.\"PG_FOREIGN_SERVER\" l"
        )
        .to_owned();
    }

    if compact
        == "selectl.oid,l.*frompg_catalog.pg_foreign_data_wrapperlleftouterjoinpg_catalog.pg_procponp.oid=l.fdwhandler"
    {
        return concat!(
            "SELECT l.oid, l.fdwname, l.fdwowner, l.fdwhandler, ",
            "l.fdwvalidator, l.fdwacl, l.fdwoptions ",
            "FROM PG_CATALOG.\"PG_FOREIGN_DATA_WRAPPER\" l ",
            "LEFT OUTER JOIN PG_CATALOG.PG_PROC p ON p.oid = l.fdwhandler"
        )
        .to_owned();
    }

    let user_mappings_prefix = concat!(
        "select distinct fs.srvname, case when rolname is null then 'public' else rolname end ",
        "rolname, srvoptions, umoptions from pg_user_mappings um join pg_foreign_server fs ",
        "on um.srvid = fs.oid left join pg_authid pa on um.umuser = pa.oid where fs.oid = "
    );
    if normalized.starts_with(user_mappings_prefix) && normalized.ends_with(" order by srvname") {
        let oid = normalized
            .trim_start_matches(user_mappings_prefix)
            .trim_end_matches(" order by srvname")
            .trim();
        return concat!(
            "SELECT DISTINCT fs.srvname, ",
            "CASE WHEN rolname IS NULL THEN 'public' ELSE rolname END AS rolname, ",
            "srvoptions, umoptions ",
            "FROM PG_CATALOG.PG_USER_MAPPINGS um ",
            "JOIN PG_CATALOG.\"PG_FOREIGN_SERVER\" fs ON um.srvid = fs.oid ",
            "LEFT JOIN PG_CATALOG.PG_AUTHID pa ON um.umuser = pa.oid ",
            "WHERE fs.oid = "
        )
        .to_owned()
            + oid
            + " ORDER BY srvname";
    }

    if normalized.starts_with("with table_privileges as (")
        && normalized.contains("has_any_column_privilege")
        && normalized.contains("has_table_privilege")
        && normalized.contains("from table_privileges")
    {
        return r#"
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
WHERE LOWER(object_schema) NOT LIKE 'pg\_%'
  AND LOWER(object_schema) <> 'information_schema'
"#
        .to_owned();
    }

    if normalized.contains("from information_schema.columns")
        && normalized.contains("col_description")
        && normalized.contains("union all")
        && normalized.contains("from pg_catalog.pg_class")
    {
        let schema_filter = extract_in_filter(sql, "c.table_schema", "C.TABLE_SCHEMA");
        let table_filter = extract_in_filter(sql, "c.table_name", "C.TABLE_NAME");
        return format!(
            r#"
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
"#
        );
    }

    if normalized.contains("from pg_constraint")
        && normalized.contains("fk-table-schema")
        && normalized.contains("pk-table-schema")
        && normalized.contains("any(c.conkey)")
    {
        let schema_filter = extract_in_filter(sql, "fk_ns.nspname", "CC.CONSTRAINT_SCHEMA");
        let table_filter = extract_in_filter(sql, "fk_table.relname", "CC.CONSTRAINT_TABLE");
        return format!(
            r#"
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
"#
        );
    }

    if normalized.contains("information_schema._pg_expandarray")
        && normalized.contains("pg_index")
        && normalized.contains("i.indisprimary")
        && normalized.contains("key_seq")
        && normalized.contains("pk_name")
    {
        let schema_filter = extract_in_or_eq_filter(sql, "n.nspname", "CC.CONSTRAINT_SCHEMA");
        let table_filter = extract_in_or_eq_filter(sql, "ct.relname", "CC.CONSTRAINT_TABLE");
        return format!(
            r#"
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
"#
        );
    }

    if normalized.contains("information_schema._pg_expandarray")
        && normalized.contains("pg_get_indexdef")
        && normalized.contains("pg_catalog.pg_index")
    {
        return r#"
SELECT
    CAST(NULL AS VARCHAR(128)) AS "table-schema",
    CAST(NULL AS VARCHAR(128)) AS "table-name",
    CAST(NULL AS VARCHAR(128)) AS "field-name"
FROM (SELECT 1 AS DUMMY)
WHERE 1 = 0
"#
        .to_owned();
    }

    if normalized.contains("from pg_constraint")
        && normalized.contains("lateral unnest")
        && normalized.contains("array_agg(col.attname")
    {
        let schema_filter = extract_like_filter(sql, "sch.nspname", "C.CONSTRAINT_SCHEMA")
            .unwrap_or_else(|| "1 = 1".to_owned());
        let table_filter = extract_like_filter(sql, "tbl.relname", "C.CONSTRAINT_TABLE")
            .unwrap_or_else(|| "1 = 1".to_owned());
        return format!(
            r#"
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
"#
        );
    }

    if normalized.contains("from pg_catalog.pg_trigger")
        && normalized.contains("information_schema.triggers")
        && normalized.contains("array_agg(")
    {
        return r#"
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
"#
        .to_owned();
    }

    sql.to_owned()
}

fn rewrite_pg_catalog(sql: &str) -> String {
    let mut sql = normalize_ansi_quoted_postgres_identifiers(sql);
    sql = rewrite_quote_ident(&sql);
    sql = rewrite_qualified_operators(&sql);
    sql = rewrite_object_description(&sql);
    sql = PG_IDENTIFY_OBJECT_IDENTITY_RE
        .replace_all(&sql, |cap: &Captures| {
            format!(
                "PG_CATALOG.PG_IDENTIFY_OBJECT({}, {}, {})",
                cap[1].trim(),
                cap[2].trim(),
                cap[3].trim()
            )
        })
        .to_string();
    sql = PG_GET_VIEWDEF_PRETTY_RE
        .replace_all(&sql, "PG_CATALOG.PG_GET_VIEWDEF($1)")
        .to_string();
    sql = PG_GET_EXPR_PRETTY_RE
        .replace_all(&sql, "PG_CATALOG.PG_GET_EXPR($1, $2)")
        .to_string();
    sql = PG_GET_CONSTRAINTDEF_PRETTY_RE
        .replace_all(&sql, "PG_CATALOG.PG_GET_CONSTRAINTDEF($1)")
        .to_string();
    sql = CURRENT_DATABASE_RE
        .replace_all(&sql, "'exasol'")
        .to_string();
    sql = CURRENT_CATALOG_RE.replace_all(&sql, "'exasol'").to_string();
    // Exasol exposes CURRENT_SCHEMA as a keyword, not a function. PostgreSQL
    // clients send `current_schema()` (with parens); drop them.
    sql = CURRENT_SCHEMA_CALL_RE
        .replace_all(&sql, "CURRENT_SCHEMA")
        .to_string();
    sql = CURRENT_SCHEMAS_FIRST_RE
        .replace_all(&sql, "'PG_CATALOG'")
        .to_string();
    sql = qualify_schema_prefix(&sql, "pg_catalog", "PG_CATALOG");
    sql = qualify_schema_prefix(&sql, "information_schema", "INFORMATION_SCHEMA");
    sql = rewrite_catalog_relations(&sql);
    for (source, target) in FUNCTION_REPLACEMENTS {
        sql = qualify_function(&sql, source, target);
    }
    rewrite_regex_operators(&rewrite_regclass_literals(&sql))
}

fn rewrite_catalog_relations(sql: &str) -> String {
    CATALOG_RELATION_RE
        .replace_all(sql, |cap: &Captures| {
            let prefix = cap.get(1).map_or("", |m| m.as_str());
            let relation = cap.name("rel").map_or("", |m| m.as_str());
            let suffix = cap.get(3).map_or("", |m| m.as_str());
            let upper = relation.to_ascii_uppercase();
            if relation.eq_ignore_ascii_case("pg_foreign_server")
                || relation.eq_ignore_ascii_case("pg_foreign_data_wrapper")
            {
                format!("{prefix}PG_CATALOG.\"{upper}\"{suffix}")
            } else {
                format!("{prefix}PG_CATALOG.{upper}{suffix}")
            }
        })
        .to_string()
}

fn rewrite_sqlglot_edge_cases(sql: &str) -> String {
    let projection = INFORMATION_SCHEMA_COLUMNS
        .iter()
        .map(|column| format!("\"{column}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = sql
        .replace(
            "PG_CATALOG.\"PG_FOREIGN_SERVER\" AS fs",
            "PG_CATALOG.\"PG_FOREIGN_SERVER\" AS srv",
        )
        .replace(
            "ARRAY_AGG(CAST(event_manipulation AS LONG VARCHAR))",
            "LISTAGG(CAST(event_manipulation AS VARCHAR(2000000)), ', ') WITHIN GROUP (ORDER BY event_manipulation)",
        )
        .replace(
            " WHERE p.prorettype <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype) AND (p.proargtypes[-1] IS NULL OR p.proargtypes[-1] <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype)) AND",
            " WHERE",
        )
        .replace(
            " WHERE p.prorettype <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype) AND (p.proargtypes[-1] IS NULL OR p.proargtypes[-1] <> CAST('PG_CATALOG.cstring' AS PG_CATALOG.regtype))",
            " WHERE 1 = 1",
        )
        .replace(
            "CASE p.proargtypes[-1] WHEN CAST('PG_CATALOG.\"any\"' AS PG_CATALOG.regtype) THEN CAST('(all types)' AS PG_CATALOG.text) ELSE PG_CATALOG.format_type(p.proargtypes[-1], NULL) END",
            "PG_CATALOG.OIDVECTORTYPES(p.proargtypes)",
        )
        .replace(
            "(t.typrelid = 0 OR (SELECT c.relkind = 'c' FROM PG_CATALOG.pg_class AS c WHERE c.oid = t.typrelid))",
            "(t.typrelid = 0)",
        )
        .replace(
            "ON (C.TABLE_CATALOG, C.TABLE_SCHEMA, C.TABLE_NAME, 'TABLE', C.DTD_IDENTIFIER) = (E.OBJECT_CATALOG, E.OBJECT_SCHEMA, E.OBJECT_NAME, E.OBJECT_TYPE, E.DTD_IDENTIFIER)",
            "ON C.TABLE_CATALOG = E.OBJECT_CATALOG AND C.TABLE_SCHEMA = E.OBJECT_SCHEMA AND C.TABLE_NAME = E.OBJECT_NAME AND E.OBJECT_TYPE = 'TABLE' AND C.DTD_IDENTIFIER = E.DTD_IDENTIFIER",
        )
        .replace(
            "ON (C.TABLE_CATALOG, C.TABLE_SCHEMA, C.TABLE_NAME, C.COLUMN_NAME, 'column_name') = (CO.TABLE_CATALOG, CO.TABLE_SCHEMA, CO.TABLE_NAME, CO.COLUMN_NAME, CO.OPTION_NAME)",
            "ON C.TABLE_CATALOG = CO.TABLE_CATALOG AND C.TABLE_SCHEMA = CO.TABLE_SCHEMA AND C.TABLE_NAME = CO.TABLE_NAME AND C.COLUMN_NAME = CO.COLUMN_NAME AND CO.OPTION_NAME = 'column_name'",
        )
        .replace(
            "CO.OPTION_VALUE AS COLUMN_OPTION, C.ORDINAL_POSITION, C.IS_IDENTITY",
            "CO.OPTION_VALUE AS COLUMN_OPTION, C.ORDINAL_POSITION AS ORDINAL_POSITION_DUP, C.IS_IDENTITY",
        );
    sql = replace_case_type_name_star(&sql, &projection);
    sql = Regex::new(r"(?i)\bAS\s+VARCHAR\s*\)")
        .unwrap()
        .replace_all(&sql, "AS VARCHAR(2000000))")
        .to_string();
    if sql.contains("PG_CATALOG.\"PG_FOREIGN_SERVER\" AS srv") {
        sql = Regex::new(r"(?i)\bfs\.")
            .unwrap()
            .replace_all(&sql, "srv.")
            .to_string();
    }
    sql
}

fn replace_case_type_name_star(sql: &str, projection: &str) -> String {
    Regex::new(
        r"(?is)\bEND\s+AS\s+type_name\s*,\s*(?:[A-Z_][A-Z0-9_]*\.)?\*\s+FROM\s+INFORMATION_SCHEMA\.COLUMNS\b",
    )
    .unwrap()
    .replace_all(sql, format!("END AS type_name, {projection} FROM INFORMATION_SCHEMA.COLUMNS"))
    .to_string()
}

fn rewrite_object_description(sql: &str) -> String {
    OBJ_DESCRIPTION_RE
        .replace_all(sql, |cap: &Captures| {
            let classoid = if cap[2].eq_ignore_ascii_case("pg_namespace") {
                "2615"
            } else {
                "1259"
            };
            format!(
                "(SELECT D.DESCRIPTION FROM PG_CATALOG.PG_DESCRIPTION D WHERE D.OBJOID = {} AND D.CLASSOID = {classoid} AND D.OBJSUBID = 0)",
                cap[1].trim()
            )
        })
        .to_string()
}

fn rewrite_regex_operators(sql: &str) -> String {
    let sql = REGEX_NOT_MATCH_RE
        .replace_all(sql, "REGEXP_INSTR($1, $2) = 0")
        .to_string();
    REGEX_MATCH_RE
        .replace_all(&sql, "REGEXP_INSTR($1, $2) > 0")
        .to_string()
}

fn rewrite_regclass_literals(sql: &str) -> String {
    REGCLASS_LITERAL_RE
        .replace_all(sql, |cap: &Captures| {
            match cap[1]
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "pg_class" => "1259".to_owned(),
                "pg_database" => "1262".to_owned(),
                "pg_namespace" => "2615".to_owned(),
                "pg_proc" => "1255".to_owned(),
                "pg_type" => "1247".to_owned(),
                _ => "0".to_owned(),
            }
        })
        .to_string()
}

fn rewrite_qualified_operators(sql: &str) -> String {
    QUALIFIED_OPERATOR_RE
        .replace_all(sql, |cap: &Captures| format!(" {} ", &cap[1]))
        .to_string()
}

fn rewrite_quote_ident(sql: &str) -> String {
    // Exasol has no quote_ident; emulate by always wrapping the argument in
    // double quotes and doubling any embedded double quotes. Matches the
    // contract Metabase relies on (output is embedded as an identifier).
    QUOTE_IDENT_RE
        .replace_all(sql, |cap: &Captures| {
            format!("('\"' || REPLACE({}, '\"', '\"\"') || '\"')", cap[1].trim())
        })
        .to_string()
}

fn rewrite_ilike(sql: &str) -> String {
    ILIKE_RE
        .replace_all(sql, "UPPER($1) LIKE UPPER($2)")
        .to_string()
}

fn qualify_schema_prefix(sql: &str, source: &str, target: &str) -> String {
    Regex::new(&format!(r"(?i)\b{}\.", regex::escape(source)))
        .unwrap()
        .replace_all(sql, format!("{target}."))
        .to_string()
}

fn qualify_function(sql: &str, source: &str, target: &str) -> String {
    let pattern = Regex::new(&format!(r"(?i)\b{}\s*\(", regex::escape(source))).unwrap();
    let mut out = String::with_capacity(sql.len());
    let mut last = 0;
    for mat in pattern.find_iter(sql) {
        let before = sql[..mat.start()].chars().last();
        if matches!(before, Some('.') | Some('"')) || before.is_some_and(is_ident_char) {
            continue;
        }
        out.push_str(&sql[last..mat.start()]);
        out.push_str(target);
        out.push('(');
        last = mat.end();
    }
    out.push_str(&sql[last..]);
    out
}

fn extract_in_filter(sql: &str, source_column: &str, target_column: &str) -> String {
    let pattern = Regex::new(&format!(
        r"(?is)\b{}\s+IN\s*\(([^)]*)\)",
        regex::escape(source_column)
    ))
    .unwrap();
    pattern
        .captures(sql)
        .map(|cap| format!(" AND {target_column} IN ({})", cap[1].trim()))
        .unwrap_or_default()
}

fn extract_eq_filter(sql: &str, source_column: &str, target_column: &str) -> String {
    let pattern = Regex::new(&format!(
        r"(?is)\b{}\s*=\s*('(?:''|[^'])*'|[A-Za-z_][A-Za-z0-9_.]*)",
        regex::escape(source_column)
    ))
    .unwrap();
    pattern
        .captures(sql)
        .map(|cap| format!(" AND {target_column} = {}", cap[1].trim()))
        .unwrap_or_default()
}

fn extract_in_or_eq_filter(sql: &str, source_column: &str, target_column: &str) -> String {
    let filter = extract_in_filter(sql, source_column, target_column);
    if filter.is_empty() {
        extract_eq_filter(sql, source_column, target_column)
    } else {
        filter
    }
}

fn extract_like_filter(sql: &str, source_column: &str, target_column: &str) -> Option<String> {
    let pattern = Regex::new(&format!(
        r"(?is)\b{}\s+LIKE\s+('(?:''|[^'])*')",
        regex::escape(source_column)
    ))
    .unwrap();
    pattern
        .captures(sql)
        .map(|cap| format!("{target_column} LIKE {}", cap[1].trim()))
}

fn normalize_for_match(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    let Some(mut start) = haystack.find(needle) else {
        return false;
    };

    while let Some(match_start) = haystack[start..].find(needle).map(|idx| start + idx) {
        let match_end = match_start + needle.len();
        let before = haystack[..match_start].chars().next_back();
        let after = haystack[match_end..].chars().next();
        if !before.is_some_and(is_ident_char) && !after.is_some_and(is_ident_char) {
            return true;
        }
        start = match_end;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_postgres_qualified_operator_syntax() {
        let sql = "SELECT a.attname FROM PG_CATALOG.pg_attribute AS a JOIN PG_CATALOG.pg_class AS c ON a.attrelid OPERATOR(PG_CATALOG.=) c.oid WHERE a.attnum OPERATOR(PG_CATALOG.>) 0";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        let normalized = translated.to_ascii_uppercase();
        assert!(!translated.to_ascii_uppercase().contains("OPERATOR("));
        assert!(normalized.contains("A.ATTRELID = C.OID"));
        assert!(normalized.contains("A.ATTNUM > 0"));
    }

    #[test]
    fn leaves_metabase_mbql_group_by_query_as_exasol_sql() {
        let sql = r#"-- Metabase:: userID: 1 queryType: MBQL queryHash: af2fd35a339c158f6a0f1f9e64b8760fdc04c809ac6172534676446a6d808b8d
SELECT "CORE_DB_2026_1_DEMOS"."SHIPMENTS"."CARRIER" AS "CARRIER", COUNT(*) AS "count" FROM "CORE_DB_2026_1_DEMOS"."SHIPMENTS" GROUP BY "CORE_DB_2026_1_DEMOS"."SHIPMENTS"."CARRIER" ORDER BY "CORE_DB_2026_1_DEMOS"."SHIPMENTS"."CARRIER" ASC"#;

        let translated = translate_postgres_to_exasol(sql).unwrap();

        assert_eq!(translated, sql);
    }

    #[test]
    fn leaves_large_exasol_compatible_query_as_exasol_sql() {
        let mut values = Vec::new();
        for idx in 0..4_000 {
            values.push(format!("'shipment-{idx:04}'"));
        }
        let sql = format!(
            "SELECT \"CORE_DB_2026_1_DEMOS\".\"SHIPMENTS\".\"CARRIER\" AS \"CARRIER\", \
             COUNT(*) AS \"count\" FROM \"CORE_DB_2026_1_DEMOS\".\"SHIPMENTS\" \
             WHERE \"CORE_DB_2026_1_DEMOS\".\"SHIPMENTS\".\"TRACKING_ID\" IN ({}) \
             GROUP BY \"CORE_DB_2026_1_DEMOS\".\"SHIPMENTS\".\"CARRIER\" \
             ORDER BY \"CORE_DB_2026_1_DEMOS\".\"SHIPMENTS\".\"CARRIER\" ASC",
            values.join(", ")
        );

        assert!(sql.len() > 50_000);
        let translated = translate_postgres_to_exasol(&sql).unwrap();

        assert_eq!(translated, sql);
    }

    #[test]
    fn rewrites_metabase_foreign_key_query() {
        let sql = r#"
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
"#;
        let translated = translate_postgres_to_exasol(sql).unwrap();
        assert!(translated.contains("SYS.EXA_DBA_CONSTRAINT_COLUMNS"));
        assert!(!translated.to_ascii_uppercase().contains("ANY("));
        assert!(translated.contains("CC.CONSTRAINT_SCHEMA IN ('NYC_UBER')"));
    }

    #[test]
    fn rewrites_qlik_column_projection() {
        let sql = "SELECT table_catalog AS TABLE_CAT FROM (SELECT CASE udt_name WHEN 'boolean' THEN 'bool' ELSE udt_name END AS type_name, * FROM INFORMATION_SCHEMA.COLUMNS WHERE table_schema LIKE 'DEMO_FINANCE'::varchar) S";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        assert!(
            !translated
                .to_ascii_uppercase()
                .contains("TYPE_NAME, * FROM INFORMATION_SCHEMA.COLUMNS")
        );
        assert!(translated.contains("\"TABLE_CATALOG\""));
        assert!(translated.contains("VARCHAR(2000000)"));
    }

    #[test]
    fn rewrites_dbvis_tablespace_helpers() {
        let sql = "SELECT pg_size_pretty(pg_tablespace_size(spcname)), pg_tablespace_location(ts.oid) FROM pg_catalog.pg_tablespace ts";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        assert!(translated.contains("PG_CATALOG.PG_SIZE_PRETTY"));
        assert!(translated.contains("PG_CATALOG.PG_TABLESPACE_SIZE"));
        assert!(translated.contains("PG_CATALOG.PG_TABLESPACE_LOCATION"));
    }

    #[test]
    fn rewrites_ilike_literal_operands() {
        let translated = translate_postgres_to_exasol("SELECT 'Alice' ILIKE 'ali%'").unwrap();
        assert_eq!(translated, "SELECT UPPER('Alice') LIKE UPPER('ali%')");
    }

    #[test]
    fn rewrites_collations_projection() {
        let translated = translate_postgres_to_exasol(
            "SELECT COLLATION_SCHEMA, COLLATION_NAME FROM INFORMATION_SCHEMA.COLLATIONS",
        )
        .unwrap();
        assert_eq!(
            translated,
            "SELECT \"COLLATION_SCHEMA\", \"COLLATION_NAME\" FROM INFORMATION_SCHEMA.\"COLLATIONS\""
        );
    }

    #[test]
    fn rewrites_foreign_server_projection() {
        let translated =
            translate_postgres_to_exasol("SELECT l.oid,l.* FROM pg_catalog.pg_foreign_server l")
                .unwrap();
        assert!(translated.contains("PG_CATALOG.\"PG_FOREIGN_SERVER\""));
        assert!(translated.contains("l.srvoptions"));
        assert!(!translated.contains("l.*"));
    }

    #[test]
    fn rewrites_foreign_data_wrapper_projection() {
        let translated = translate_postgres_to_exasol("SELECT l.oid,l.* FROM pg_catalog.pg_foreign_data_wrapper l LEFT OUTER JOIN pg_catalog.pg_proc p ON p.oid=l.fdwhandler").unwrap();
        assert!(translated.contains("PG_CATALOG.\"PG_FOREIGN_DATA_WRAPPER\""));
        assert!(translated.contains("LEFT OUTER JOIN PG_CATALOG.PG_PROC"));
        assert!(translated.contains("l.fdwoptions"));
    }

    #[test]
    fn rewrites_user_mappings_query_with_dynamic_oid() {
        let translated = translate_postgres_to_exasol("select distinct fs.srvname, case when rolname is null then 'public' else rolname end rolname, srvoptions, umoptions from pg_user_mappings um join pg_foreign_server fs on um.srvid = fs.OID left join pg_authid pa on um.umuser = pa.OID where fs.OID = 42 ORDER BY srvname").unwrap();
        assert!(translated.contains("PG_CATALOG.PG_USER_MAPPINGS"));
        assert!(translated.contains("PG_CATALOG.\"PG_FOREIGN_SERVER\""));
        assert!(translated.contains("WHERE fs.oid = 42"));
    }

    #[test]
    fn rewrites_quote_ident_call_to_concat_expression() {
        let translated = translate_postgres_to_exasol("SELECT quote_ident($1)").unwrap();
        assert!(!translated.to_ascii_lowercase().contains("quote_ident"));
        assert!(translated.contains("REPLACE($1"));
        assert!(translated.contains("'\"'"));
    }

    #[test]
    fn drops_parens_on_current_schema_function_call() {
        // Exasol exposes CURRENT_SCHEMA as a keyword, not a function. PostgreSQL
        // clients (and the JDBC driver) send `current_schema()`; we must strip
        // the parens so Exasol accepts it.
        let translated = translate_postgres_to_exasol("SELECT current_schema()").unwrap();
        assert!(translated.to_ascii_uppercase().contains("CURRENT_SCHEMA"));
        assert!(!translated.contains("current_schema("));
        assert!(!translated.to_ascii_uppercase().contains("CURRENT_SCHEMA("));
    }
}
