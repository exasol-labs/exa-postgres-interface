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
static SESSION_USER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bsession_user(?:\s*\(\s*\))?").unwrap());
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
// Captures the (possibly schema-qualified, possibly double-quoted) relation
// name a client inlines as `'<name>'::regclass`, e.g. `'"TEST"."SALES_HOT"'`.
static REGCLASS_STRING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)'([^']*)'\s*::\s*regclass"#).unwrap());
// Beekeeper's foreign-key relation queries filter by the owning table
// (`n`/`t` aliases, outgoing keys) or the referenced table (`nf`/`tf`,
// incoming keys). The schema/table arrive already inlined as literals.
static FK_OUT_SCHEMA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bn\.nspname\s*=\s*'([^']*)'").unwrap());
static FK_OUT_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bt\.relname\s*=\s*'([^']*)'").unwrap());
static FK_IN_SCHEMA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bnf\.nspname\s*=\s*'([^']*)'").unwrap());
static FK_IN_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\btf\.relname\s*=\s*'([^']*)'").unwrap());
// Beekeeper's getTableCreateScript filters the columns by `c.relname = '<t>'`
// (schema comes from the shared `n.nspname` predicate).
static CREATE_SCRIPT_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bc\.relname\s*=\s*'([^']*)'").unwrap());
static QUOTE_IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bquote_ident\s*\(\s*([^()]+?)\s*\)").unwrap());
// Beekeeper's listTableColumns asks for column comments via
// `col_description(format('%I.%I', table_schema, table_name)::regclass::oid,
// ordinal_position)`. Exasol has no `format()` and no `regclass` type, so the
// whole statement fails to parse and the client shows no columns at all.
// Exasol column comments aren't reachable through this oid-based lookup anyway,
// so collapse the entire call to NULL — the columns list works, only the
// (rarely used) per-column comment is dropped.
static COL_DESCRIPTION_FORMAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:pg_catalog\.)?col_description\s*\(\s*format\s*\([^)]*\)\s*::\s*regclass\s*(?:::\s*oid\s*)?,[^)]*\)",
    )
    .unwrap()
});
// Strips a leading "exasol" catalog qualifier from 3-part references so that
// `["exasol".]schema.table` becomes `schema.table`. Each segment may be
// independently quoted.
static EXASOL_CATALOG_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(^|[\s,(\[])"?exasol"?\s*\.\s*("?[A-Za-z_][A-Za-z0-9_]*"?\s*\.\s*"?[A-Za-z_][A-Za-z0-9_]*"?)"#,
    )
    .unwrap()
});

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

/// Whether `sql` is forwarded to Exasol essentially unchanged (no dialect
/// rewriting). Used by the result layer to decide whether to fold result
/// column labels to lowercase: a passthrough query selects real Exasol columns
/// whose (upper-case) names the client must see verbatim, whereas a translated
/// catalog query carries lower-case aliases the client reads case-sensitively.
pub fn is_passthrough_query(sql: &str) -> bool {
    let sql = strip_exasol_catalog_prefix(sql);
    is_exasol_passthrough_sql(sql.as_str())
}

pub fn translate_postgres_to_exasol(sql: &str) -> Result<String, TranslationError> {
    let sql = strip_exasol_catalog_prefix(sql);
    let sql = sql.as_str();
    let translated = if is_exasol_passthrough_sql(sql) {
        quote_reserved_aliases(sql)
    } else {
        let normalized = normalize_ansi_quoted_postgres_identifiers(sql);
        let known_metadata_query = rewrite_known_metadata_query(&normalized);
        if known_metadata_query != normalized {
            quote_reserved_aliases(&rewrite_ilike(&known_metadata_query))
        } else {
            let rewritten = rewrite_pg_catalog(&normalized);
            let transpiled = polyglot_sql::transpile_by_name(&rewritten, "postgres", "exasol")
                .map_err(|err| TranslationError::new(err.to_string()))?
                .join("; ");
            let transpiled = rewrite_sqlglot_edge_cases(&transpiled);
            quote_reserved_aliases(&rewrite_ilike(&transpiled))
        }
    };
    // Applied to every path (including passthrough): a bare `OFFSET 0`/`OFFSET
    // NULL` is a no-op in PostgreSQL, but Exasol rejects any OFFSET without an
    // ORDER BY. Clients like Beekeeper send `LIMIT n OFFSET 0` for the first
    // page of a table; dropping the redundant OFFSET makes it run unchanged.
    Ok(strip_redundant_offset(&translated))
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
        " session_user",
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

    // Beekeeper Studio's table-list query joins information_schema.tables to
    // pg_class (to read relkind) and self-joins through pg_inherits to find a
    // partition/inheritance parent. The join predicate casts the namespace OID
    // with `relnamespace::regnamespace::text`, which sqlglot renders as
    // `CAST(... AS REGNAMESPACE)` — a type Exasol doesn't have, so it fails to
    // parse. Even with that cast fixed, the pg_inherits self-join has no
    // meaning on Exasol (no table inheritance or partitioning), so this query
    // can only be answered by recognizing its intent, not by rewriting its
    // expressions. We match on that structural signature — tables view +
    // pg_inherits + relkind — rather than the client's exact column aliases,
    // so alias/whitespace/casing variants still resolve. Every base table is an
    // ordinary table (relkind 'r') with no parent, sourced from
    // SYS.EXA_ALL_TABLES (tables only — views are excluded, matching the
    // original `table_type NOT LIKE '%VIEW%'` filter).
    if normalized.contains("information_schema.tables")
        && normalized.contains("pg_inherits")
        && normalized.contains("relkind")
    {
        return concat!(
            "SELECT\n",
            "    TABLE_SCHEMA AS \"schema\",\n",
            "    TABLE_NAME AS \"name\",\n",
            "    'r' AS \"tabletype\",\n",
            "    CAST(NULL AS CHAR(1)) AS \"parenttype\"\n",
            "FROM SYS.EXA_ALL_TABLES\n",
            "ORDER BY TABLE_SCHEMA, TABLE_NAME"
        )
        .to_owned();
    }

    // Beekeeper's getPrimaryKeys reads PG_INDEX/PG_ATTRIBUTE filtered by
    // `indisprimary`, with the table inlined as `'<table>'::regclass`. Exasol
    // has no regclass type, no `ANY(indkey)` array op, and no pg_index data;
    // primary-key columns live in SYS.EXA_ALL_CONSTRAINT_COLUMNS. Rewrite to a
    // native lookup returning the same {column_name, data_type, position}
    // shape (empty when the table has no primary key).
    if normalized.contains("pg_index")
        && normalized.contains("indisprimary")
        && normalized.contains("::regclass")
    {
        if let Some((schema, table)) = regclass_schema_table(sql) {
            return primary_key_query(schema.as_deref(), &table);
        }
    }

    // Beekeeper's getOutgoingKeys / getIncomingKeys enumerate foreign keys via
    // PG_CONSTRAINT + GENERATE_SUBSCRIPTS + array indexing (`conkey[pos]`),
    // none of which Exasol supports. Exasol keeps foreign keys in
    // SYS.EXA_ALL_CONSTRAINT_COLUMNS (with REFERENCED_* columns), so rewrite to
    // a native lookup. Outgoing keys filter on the owning table; incoming keys
    // on the referenced table (and label the local column `from_column`).
    if normalized.contains("pg_constraint")
        && normalized.contains("generate_subscripts")
        && normalized.contains("contype = 'f'")
    {
        if let (Some(schema), Some(table)) = (
            FK_OUT_SCHEMA_RE.captures(sql).map(|c| c[1].to_owned()),
            FK_OUT_TABLE_RE.captures(sql).map(|c| c[1].to_owned()),
        ) {
            return foreign_key_query(&schema, &table, false);
        }
        if let (Some(schema), Some(table)) = (
            FK_IN_SCHEMA_RE.captures(sql).map(|c| c[1].to_owned()),
            FK_IN_TABLE_RE.captures(sql).map(|c| c[1].to_owned()),
        ) {
            return foreign_key_query(&schema, &table, true);
        }
    }

    // Beekeeper's getTableCreateScript reconstructs a table's DDL by walking
    // PG_CLASS/PG_ATTRIBUTE/PG_ATTRDEF and stitching the column lines together
    // with `array_agg(... ORDER BY ...)` + `array_to_string` -- array functions
    // Exasol doesn't have. Rebuild the same `createtable` string natively from
    // SYS.EXA_ALL_COLUMNS (with LISTAGG) plus the primary key from
    // SYS.EXA_ALL_CONSTRAINT_COLUMNS.
    if normalized.contains("as createtable") && normalized.contains("array_agg") {
        if let (Some(table), Some(schema)) = (
            CREATE_SCRIPT_TABLE_RE.captures(sql).map(|c| c[1].to_owned()),
            FK_OUT_SCHEMA_RE.captures(sql).map(|c| c[1].to_owned()),
        ) {
            return create_table_script_query(&schema, &table);
        }
    }

    sql.to_owned()
}

/// Extracts `(schema, table)` from the relation name a client inlines as
/// `'<name>'::regclass`. Handles `"S"."T"`, `S.T`, and bare `T`.
fn regclass_schema_table(sql: &str) -> Option<(Option<String>, String)> {
    let raw = REGCLASS_STRING_RE.captures(sql)?.get(1)?.as_str();
    let cleaned = raw.replace('"', "");
    let mut parts: Vec<&str> = cleaned.split('.').collect();
    let table = parts.pop().filter(|s| !s.is_empty())?.to_string();
    let schema = parts.pop().filter(|s| !s.is_empty()).map(str::to_string);
    Some((schema, table))
}

/// Native Exasol primary-key column lookup matching Beekeeper's getPrimaryKeys
/// result shape. Aliases are quoted lowercase so the labels reach the client
/// as PostgreSQL would fold them.
fn primary_key_query(schema: Option<&str>, table: &str) -> String {
    let esc = |s: &str| s.replace('\'', "''");
    let mut filter = format!(
        "cc.CONSTRAINT_TYPE = 'PRIMARY KEY' AND cc.CONSTRAINT_TABLE = '{}'",
        esc(table)
    );
    if let Some(schema) = schema {
        filter.push_str(&format!(" AND cc.CONSTRAINT_SCHEMA = '{}'", esc(schema)));
    }
    format!(
        "SELECT cc.COLUMN_NAME AS \"column_name\", c.COLUMN_TYPE AS \"data_type\", \
         cc.ORDINAL_POSITION AS \"position\" \
         FROM SYS.EXA_ALL_CONSTRAINT_COLUMNS cc \
         LEFT JOIN SYS.EXA_ALL_COLUMNS c \
         ON c.COLUMN_SCHEMA = cc.CONSTRAINT_SCHEMA AND c.COLUMN_TABLE = cc.CONSTRAINT_TABLE \
         AND c.COLUMN_NAME = cc.COLUMN_NAME \
         WHERE {filter} ORDER BY cc.ORDINAL_POSITION"
    )
}

/// Native Exasol foreign-key lookup matching Beekeeper's getOutgoingKeys
/// (`incoming = false`) / getIncomingKeys (`incoming = true`) result shape.
/// Exasol doesn't record per-constraint update/delete actions, so both rules
/// default to `NO ACTION`. Aliases are quoted lowercase so labels reach the
/// client as PostgreSQL would fold them.
fn foreign_key_query(schema: &str, table: &str, incoming: bool) -> String {
    let esc = |s: &str| s.replace('\'', "''");
    // Outgoing keys are owned by the table; incoming keys reference it.
    let (schema_col, table_col) = if incoming {
        ("cc.REFERENCED_SCHEMA", "cc.REFERENCED_TABLE")
    } else {
        ("cc.CONSTRAINT_SCHEMA", "cc.CONSTRAINT_TABLE")
    };
    // getIncomingKeys labels the local column `from_column`; getOutgoingKeys
    // labels it `column_name`.
    let local_column_alias = if incoming { "from_column" } else { "column_name" };
    format!(
        "SELECT cc.CONSTRAINT_NAME AS \"constraint_name\", \
         cc.COLUMN_NAME AS \"{local_column_alias}\", \
         cc.CONSTRAINT_SCHEMA AS \"from_schema\", cc.CONSTRAINT_TABLE AS \"from_table\", \
         cc.REFERENCED_COLUMN AS \"to_column\", cc.REFERENCED_TABLE AS \"to_table\", \
         cc.REFERENCED_SCHEMA AS \"to_schema\", \
         'NO ACTION' AS \"update_rule\", 'NO ACTION' AS \"delete_rule\", \
         cc.ORDINAL_POSITION AS \"ordinal_position\" \
         FROM SYS.EXA_ALL_CONSTRAINT_COLUMNS cc \
         WHERE cc.CONSTRAINT_TYPE = 'FOREIGN KEY' AND {schema_col} = '{}' AND {table_col} = '{}' \
         ORDER BY cc.CONSTRAINT_NAME, cc.ORDINAL_POSITION",
        esc(schema),
        esc(table)
    )
}

/// Native reconstruction of Beekeeper's getTableCreateScript output: a single
/// `createtable` string holding the `CREATE TABLE` DDL (columns with type,
/// NOT NULL and DEFAULT) plus an `ALTER TABLE ... ADD PRIMARY KEY` when the
/// table has one. Built with LISTAGG since Exasol has no array aggregation.
/// Note: Exasol's `||` follows Oracle semantics (NULL concatenates as empty),
/// so the primary-key clause is guarded with an explicit `IS NOT NULL` CASE
/// rather than COALESCE over a concatenation.
fn create_table_script_query(schema: &str, table: &str) -> String {
    let esc = |s: &str| s.replace('\'', "''");
    let schema = esc(schema);
    let table = esc(table);
    format!(
        "SELECT 'CREATE TABLE \"' || cols.COLUMN_SCHEMA || '\".\"' || cols.COLUMN_TABLE || '\" (' \
         || CHR(10) || cols.col_defs || CHR(10) || ');' \
         || CASE WHEN pk.pk_cols IS NOT NULL THEN CHR(10) || 'ALTER TABLE \"' || cols.COLUMN_SCHEMA \
         || '\".\"' || cols.COLUMN_TABLE || '\" ADD PRIMARY KEY (' || pk.pk_cols || ');' ELSE '' END \
         AS \"createtable\" \
         FROM (SELECT COLUMN_SCHEMA, COLUMN_TABLE, \
         LISTAGG('  \"' || COLUMN_NAME || '\" ' || COLUMN_TYPE \
         || CASE WHEN COLUMN_IS_NULLABLE THEN '' ELSE ' NOT NULL' END \
         || CASE WHEN COLUMN_DEFAULT IS NOT NULL THEN ' DEFAULT ' || COLUMN_DEFAULT ELSE '' END, \
         ',' || CHR(10)) WITHIN GROUP (ORDER BY COLUMN_ORDINAL_POSITION) AS col_defs \
         FROM SYS.EXA_ALL_COLUMNS WHERE COLUMN_SCHEMA = '{schema}' AND COLUMN_TABLE = '{table}' \
         GROUP BY COLUMN_SCHEMA, COLUMN_TABLE) cols \
         LEFT JOIN (SELECT CONSTRAINT_SCHEMA, CONSTRAINT_TABLE, \
         LISTAGG('\"' || COLUMN_NAME || '\"', ', ') WITHIN GROUP (ORDER BY ORDINAL_POSITION) AS pk_cols \
         FROM SYS.EXA_ALL_CONSTRAINT_COLUMNS WHERE CONSTRAINT_TYPE = 'PRIMARY KEY' \
         AND CONSTRAINT_SCHEMA = '{schema}' AND CONSTRAINT_TABLE = '{table}' \
         GROUP BY CONSTRAINT_SCHEMA, CONSTRAINT_TABLE) pk \
         ON pk.CONSTRAINT_SCHEMA = cols.COLUMN_SCHEMA AND pk.CONSTRAINT_TABLE = cols.COLUMN_TABLE"
    )
}

fn rewrite_pg_catalog(sql: &str) -> String {
    let mut sql = normalize_ansi_quoted_postgres_identifiers(sql);
    // Drop the unsupported col_description(format(...)::regclass::oid, ...)
    // comment lookup before anything else tries to map col_description to a
    // catalog UDF or transpile the format()/regclass parts.
    sql = COL_DESCRIPTION_FORMAT_RE
        .replace_all(&sql, "CAST(NULL AS VARCHAR(2000000))")
        .to_string();
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
    // Exasol has no SESSION_USER keyword; the closest equivalent is
    // CURRENT_USER (Exasol does not distinguish session-level vs current
    // identity), so map all references through.
    sql = SESSION_USER_RE
        .replace_all(&sql, "CURRENT_USER")
        .to_string();
    // Lowercase to stay consistent with PG_CATALOG.PG_NAMESPACE.nspname,
    // which now returns 'pg_catalog' so PostgreSQL clients recognise the
    // system namespace case-sensitively.
    sql = CURRENT_SCHEMAS_FIRST_RE
        .replace_all(&sql, "'pg_catalog'")
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
        // Beekeeper Studio emits the same pattern but without `AS c`. The
        // correlated scalar subselect — `(SELECT c.relkind = 'c' FROM ...)`
        // used as a boolean — is not supported by Exasol. Dropping the
        // composite-type check (and keeping `t.typrelid = 0`) means we miss
        // showing composite types; that's an acceptable approximation for
        // metadata exploration in clients that just want a type list.
        .replace(
            "(t.typrelid = 0 OR (SELECT c.relkind = 'c' FROM PG_CATALOG.pg_class c WHERE c.oid = t.typrelid))",
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

fn strip_exasol_catalog_prefix(sql: &str) -> String {
    // PostgreSQL clients (DBeaver, Qlik, etc.) often emit three-part
    // `database.schema.table` references where the database segment is
    // `exasol`. Exasol has no catalog tier, so the leading `exasol.` must be
    // stripped before the SQL is sent on. Runs ahead of every other rewrite
    // — including the passthrough check — so even otherwise Exasol-shaped SQL
    // benefits.
    EXASOL_CATALOG_PREFIX_RE
        .replace_all(sql, |cap: &Captures| {
            format!("{}{}", cap.get(1).map_or("", |m| m.as_str()), &cap[2])
        })
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

// Exasol reserved keywords. PostgreSQL clients routinely emit these as bare
// column aliases (`AS time`, `AS schema`, `AS position`, `AS condition`, ...);
// they parse fine in PostgreSQL but Exasol rejects an unquoted identifier that
// collides with a reserved keyword. This is the authoritative reserved set
// from Exasol's `EXA_SQL_KEYWORDS` system table (`WHERE reserved = TRUE`) --
// regenerate from there when upgrading Exasol. Lowercase entries; comparison
// is case-insensitive. Non-reserved identifiers that Exasol merely upper-cases
// (e.g. `name`, `id`) don't error; their labels are folded back to lowercase
// at the result layer instead.
static RESERVED_ALIAS_WORDS: LazyLock<std::collections::HashSet<&'static str>> =
    LazyLock::new(|| {
        [
            "absolute", "action", "add", "after", "all", "allocate", "alter", "and",
            "any", "append", "are", "array", "as", "asc", "asensitive", "assertion",
            "at", "attribute", "authid", "authorization", "before", "begin", "between", "bigint",
            "binary", "bit", "blob", "blocked", "bool", "boolean", "both", "by",
            "byte", "call", "called", "cardinality", "cascade", "cascaded", "case", "casespecific",
            "cast", "catalog", "chain", "char", "character", "character_set_catalog", "character_set_name", "character_set_schema",
            "characteristics", "check", "checked", "clob", "close", "coalesce", "collate", "collation",
            "collation_catalog", "collation_name", "collation_schema", "column", "commit", "condition", "connect_by_iscycle", "connect_by_isleaf",
            "connect_by_root", "connection", "constant", "constraint", "constraint_state_default", "constraints", "constructor", "contains",
            "continue", "control", "convert", "corresponding", "create", "cs", "csv", "cube",
            "current", "current_cluster", "current_cluster_uid", "current_date", "current_path", "current_role", "current_schema", "current_session",
            "current_statement", "current_time", "current_timestamp", "current_user", "cursor", "cycle", "data", "datalink",
            "date", "datetime_interval_code", "datetime_interval_precision", "day", "dbtimezone", "deallocate", "dec", "decimal",
            "declare", "default", "default_like_escape_character", "deferrable", "deferred", "defined", "definer", "delete",
            "deref", "derived", "desc", "describe", "descriptor", "deterministic", "disable", "disabled",
            "disconnect", "dispatch", "distinct", "dlurlcomplete", "dlurlpath", "dlurlpathonly", "dlurlscheme", "dlurlserver",
            "dlvalue", "do", "domain", "double", "drop", "dynamic", "dynamic_function", "dynamic_function_code",
            "each", "else", "elseif", "elsif", "emits", "enable", "enabled", "end",
            "end-exec", "endif", "enforce", "equals", "errors", "escape", "except", "exception",
            "exec", "execute", "exists", "exit", "export", "external", "extract", "false",
            "fbv", "fetch", "file", "final", "first", "float", "following", "for",
            "forall", "force", "format", "found", "free", "from", "fs", "full",
            "function", "general", "generated", "geometry", "get", "global", "go", "goto",
            "grant", "granted", "group", "group_concat", "grouping", "groups", "hashtype", "hashtype_format",
            "having", "high", "hold", "hour", "identity", "if", "ifnull", "immediate",
            "impersonate", "implementation", "import", "in", "index", "indicator", "inner", "inout",
            "input", "insensitive", "insert", "instance", "instantiable", "int", "integer", "integrity",
            "intersect", "interval", "into", "inverse", "invoker", "is", "iterate", "join",
            "key_member", "key_type", "large", "last", "lateral", "ldap", "leading", "leave",
            "left", "level", "like", "limit", "listagg", "local", "localtime", "localtimestamp",
            "locator", "log", "longvarchar", "loop", "low", "map", "match", "matched",
            "merge", "method", "minus", "minute", "mod", "modifies", "modify", "module",
            "month", "names", "national", "natural", "nchar", "nclob", "new", "next",
            "nls_date_format", "nls_date_language", "nls_first_day_of_week", "nls_numeric_characters", "nls_timestamp_format", "no", "nocycle", "nologging",
            "none", "not", "null", "nullif", "number", "numeric", "nvarchar", "nvarchar2",
            "object", "of", "off", "old", "on", "only", "open", "option",
            "options", "or", "order", "ordering", "ordinality", "others", "out", "outer",
            "output", "over", "overlaps", "overlay", "overriding", "pad", "parallel_enable", "parameter",
            "parameter_specific_catalog", "parameter_specific_name", "parameter_specific_schema", "parquet", "partial", "path", "permission", "placing",
            "plus", "position", "preceding", "preferring", "prepare", "preserve", "prior", "privileges",
            "procedure", "profile", "qualify", "random", "range", "read", "reads", "real",
            "recovery", "recursive", "ref", "references", "referencing", "refresh", "regexp_like", "relative",
            "release", "rename", "repeat", "replace", "restore", "restrict", "result", "return",
            "returned_length", "returned_octet_length", "returns", "revoke", "right", "rollback", "rollup", "routine",
            "row", "rows", "rowtype", "savepoint", "schema", "scope", "scope_user", "script",
            "scroll", "search", "second", "section", "security", "select", "selective", "self",
            "sensitive", "separator", "sequence", "session", "session_user", "sessiontimezone", "set", "sets",
            "shortint", "similar", "smallint", "some", "source", "space", "specific", "specifictype",
            "sql", "sql_bigint", "sql_bit", "sql_char", "sql_date", "sql_decimal", "sql_double", "sql_float",
            "sql_integer", "sql_longvarchar", "sql_numeric", "sql_preprocessor_script", "sql_real", "sql_smallint", "sql_timestamp", "sql_tinyint",
            "sql_type_date", "sql_type_timestamp", "sql_varchar", "sqlexception", "sqlstate", "sqlwarning", "start", "state",
            "statement", "static", "structure", "style", "substring", "subtype", "sysdate", "system",
            "system_user", "systimestamp", "table", "temporary", "text", "then", "time", "timestamp",
            "timezone_hour", "timezone_minute", "tinyint", "to", "trailing", "transaction", "transform", "transforms",
            "translation", "treat", "trigger", "trim", "true", "truncate", "under", "union",
            "unique", "unknown", "unlink", "unnest", "until", "update", "usage", "user",
            "using", "value", "values", "varchar", "varchar2", "varray", "verify", "view",
            "when", "whenever", "where", "while", "window", "with", "within", "without",
            "work", "year", "yes", "zone",
        ]
        .into_iter()
        .collect()
    });

// Matches `AS <ident>` plus any trailing whitespace and an optional bracket.
// The trailing group is how we tell a real alias (`SELECT x AS time FROM ...`)
// from a cast type name, which is never a candidate for quoting: a trailing
// `)` means `CAST(x AS DATE)` and a trailing `(` means a parameterised type
// like `CAST(x AS VARCHAR(255))`. An alias is never immediately followed by a
// bracket, so either one rules out quoting. (Rust's `regex` crate has no
// lookahead, so we capture instead of peeking.)
static ALIAS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bAS\s+([A-Za-z_][A-Za-z0-9_]*)(\s*[()]?)").unwrap());

/// Quote any `AS <ident>` whose identifier is an Exasol reserved word.
///
/// Applied to both passthrough and translated SQL so a hand-written
/// `SELECT 1 AS time` and a translated `SELECT CURRENT_SCHEMA AS schema` are
/// both made safe before being sent to Exasol.
fn quote_reserved_aliases(sql: &str) -> String {
    ALIAS_RE
        .replace_all(sql, |cap: &Captures| {
            let whole = cap.get(0).map(|m| m.as_str()).unwrap_or("");
            let ident = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let trailing = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            if trailing.contains(')') || trailing.contains('(') {
                // CAST(... AS TYPE) or CAST(... AS TYPE(n)) — a type name, not
                // an alias; leave it alone.
                return whole.to_owned();
            }
            if RESERVED_ALIAS_WORDS.contains(ident.to_ascii_lowercase().as_str()) {
                format!("AS \"{ident}\"{trailing}")
            } else {
                whole.to_owned()
            }
        })
        .to_string()
}

fn rewrite_ilike(sql: &str) -> String {
    ILIKE_RE
        .replace_all(sql, "UPPER($1) LIKE UPPER($2)")
        .to_string()
}

// `OFFSET 0` / `OFFSET NULL` (optionally `ROW`/`ROWS`) is a no-op in
// PostgreSQL, but Exasol rejects any OFFSET that isn't paired with an ORDER BY.
// Only the redundant zero/NULL form is stripped; a non-zero OFFSET is a real
// operation and is left untouched (it legitimately needs an ORDER BY).
static REDUNDANT_OFFSET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+OFFSET\s+(?:0|NULL)\b(?:\s+ROWS?\b)?").unwrap());

fn strip_redundant_offset(sql: &str) -> String {
    REDUNDANT_OFFSET_RE.replace_all(sql, "").to_string()
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
    fn strips_exasol_catalog_prefix_from_three_part_quoted_reference() {
        let sql = "SELECT \"SHIPMENT_ID\" FROM \"exasol\".\"CORE_DB_2026_1_DEMOS\".\"SHIPMENTS\"";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        assert!(!translated.contains("\"exasol\""));
        assert!(translated.contains("\"CORE_DB_2026_1_DEMOS\".\"SHIPMENTS\""));
    }

    #[test]
    fn strips_exasol_catalog_prefix_from_three_part_unquoted_reference() {
        let translated =
            translate_postgres_to_exasol("SELECT * FROM exasol.pg_demo.orders").unwrap();
        assert!(!translated.to_ascii_lowercase().contains("exasol.pg_demo"));
        assert!(translated.to_ascii_lowercase().contains("pg_demo.orders"));
    }

    #[test]
    fn leaves_unrelated_three_part_reference_alone() {
        // Reference whose first segment is not `exasol` must pass through.
        let sql = "SELECT * FROM otherdb.pg_demo.orders";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        assert!(translated.contains("otherdb.pg_demo.orders"));
    }

    #[test]
    fn rewrites_session_user_to_current_user() {
        // Exasol has no SESSION_USER keyword; PostgreSQL clients (notably
        // DBeaver and pgJDBC) reach for it during connection setup. Map both
        // the bare keyword and the parenthesised form.
        let translated = translate_postgres_to_exasol("SELECT session_user").unwrap();
        let upper = translated.to_ascii_uppercase();
        assert!(upper.contains("CURRENT_USER"));
        assert!(!upper.contains("SESSION_USER"));

        let translated = translate_postgres_to_exasol("SELECT SESSION_USER()").unwrap();
        let upper = translated.to_ascii_uppercase();
        assert!(upper.contains("CURRENT_USER"));
        assert!(!upper.contains("SESSION_USER"));
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

    #[test]
    fn quotes_reserved_word_aliases_after_as() {
        // Beekeeper sends `SELECT CURRENT_SCHEMA() AS schema` on connect;
        // SCHEMA is reserved in Exasol so the unquoted alias must be quoted.
        let translated = translate_postgres_to_exasol("SELECT CURRENT_SCHEMA() AS schema").unwrap();
        assert!(translated.contains("AS \"schema\""));

        // Common Grafana / hand-written patterns.
        assert!(
            translate_postgres_to_exasol("SELECT 1 AS time FROM dual")
                .unwrap()
                .contains("AS \"time\"")
        );
        assert!(
            translate_postgres_to_exasol("SELECT 1 AS value")
                .unwrap()
                .contains("AS \"value\"")
        );
    }

    #[test]
    fn alias_quoting_does_not_touch_cast_type_argument() {
        // `CAST(x AS DATE)` and `x::DATE` are type casts, not aliases.
        // DATE is in the reserved set but must NOT be quoted inside a cast.
        let translated =
            translate_postgres_to_exasol("SELECT CAST(ORDER_TS AS DATE) AS d FROM orders").unwrap();
        let upper = translated.to_ascii_uppercase();
        assert!(upper.contains("AS DATE)"));
        assert!(!translated.contains("AS \"DATE\")"));
        assert!(!translated.contains("AS \"date\")"));
    }

    #[test]
    fn alias_quoting_skips_non_reserved_aliases() {
        // `revenue`, `customer`, `region` are not reserved in Exasol — leave them.
        let translated =
            translate_postgres_to_exasol("SELECT amount AS revenue, name AS customer FROM orders")
                .unwrap();
        assert!(translated.contains("AS revenue"));
        assert!(translated.contains("AS customer"));
        assert!(!translated.contains("AS \"revenue\""));
    }

    #[test]
    fn alias_quoting_preserves_already_quoted_aliases() {
        // `AS "time"` is already valid; the regex requires an unquoted start
        // so it should be left alone.
        let translated = translate_postgres_to_exasol("SELECT 1 AS \"time\"").unwrap();
        // Should not contain double-quoting.
        assert!(!translated.contains("AS \"\"time\""));
        assert!(translated.contains("AS \"time\""));
    }

    #[test]
    fn strips_redundant_zero_offset() {
        // OFFSET 0 / OFFSET NULL are no-ops in PostgreSQL but Exasol rejects an
        // OFFSET without ORDER BY; drop the redundant clause.
        assert_eq!(
            translate_postgres_to_exasol("SELECT * FROM \"S\".\"T\" LIMIT 100 OFFSET 0").unwrap(),
            "SELECT * FROM \"S\".\"T\" LIMIT 100"
        );
        assert_eq!(
            translate_postgres_to_exasol("SELECT * FROM \"S\".\"T\" LIMIT 100 OFFSET NULL").unwrap(),
            "SELECT * FROM \"S\".\"T\" LIMIT 100"
        );
        // A non-zero OFFSET is a real operation and must be preserved.
        assert!(
            translate_postgres_to_exasol("SELECT * FROM \"S\".\"T\" ORDER BY 1 LIMIT 100 OFFSET 50")
                .unwrap()
                .contains("OFFSET 50")
        );
    }

    #[test]
    fn rewrites_beekeeper_create_table_script_query() {
        let sql = "SELECT 'CREATE TABLE ' || quote_ident(tabdef.schema_name) \
            || array_to_string(array_agg('  ' || quote_ident(tabdef.column_name) ORDER BY tabdef.column_idx ASC), ',') AS createtable \
            FROM ( SELECT c.relname AS table_name, a.attname AS column_name, n.nspname as schema_name \
            FROM pg_class c JOIN pg_namespace n ON (n.oid = c.relnamespace) JOIN pg_attribute a ON (a.attrelid = c.oid) \
            WHERE c.relname = 'EXA_CANDIDATE_QUEUE' AND n.nspname = 'EXA_INVESTIGATION' ORDER BY a.attnum DESC ) AS tabdef \
            GROUP BY tabdef.schema_name, tabdef.table_name";
        let out = translate_postgres_to_exasol(sql).unwrap();
        let upper = out.to_ascii_uppercase();
        assert!(!upper.contains("ARRAY_AGG") && !upper.contains("ARRAY_TO_STRING"), "got: {out}");
        assert!(!upper.contains("PG_CLASS"), "got: {out}");
        assert!(upper.contains("FROM SYS.EXA_ALL_COLUMNS"));
        assert!(upper.contains("LISTAGG"));
        assert!(out.contains("AS \"createtable\""));
        assert!(out.contains("COLUMN_SCHEMA = 'EXA_INVESTIGATION'") && out.contains("COLUMN_TABLE = 'EXA_CANDIDATE_QUEUE'"), "got: {out}");
    }

    #[test]
    fn rewrites_beekeeper_foreign_key_queries() {
        let outgoing = "SELECT c.conname AS constraint_name, a.attname AS column_name, n.nspname AS from_schema, \
            t.relname AS from_table, af.attname AS to_column, tf.relname AS to_table, nf.nspname AS to_schema, \
            pos AS ordinal_position FROM pg_constraint c JOIN pg_class t ON c.conrelid = t.oid \
            JOIN pg_namespace n ON t.relnamespace = n.oid JOIN generate_subscripts(c.conkey, 1) pos ON true \
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = c.conkey[pos] \
            JOIN pg_class tf ON c.confrelid = tf.oid JOIN pg_namespace nf ON tf.relnamespace = nf.oid \
            JOIN pg_attribute af ON af.attrelid = tf.oid AND af.attnum = c.confkey[pos] \
            WHERE c.contype = 'f' AND n.nspname = 'TEST' AND t.relname = 'SALES_HOT' ORDER BY c.conname, pos";
        let out = translate_postgres_to_exasol(outgoing).unwrap();
        let upper = out.to_ascii_uppercase();
        assert!(!upper.contains("GENERATE_SUBSCRIPTS") && !upper.contains("PG_CONSTRAINT"), "got: {out}");
        assert!(upper.contains("EXA_ALL_CONSTRAINT_COLUMNS"));
        assert!(out.contains("CONSTRAINT_TYPE = 'FOREIGN KEY'"));
        assert!(out.contains("cc.CONSTRAINT_SCHEMA = 'TEST'") && out.contains("cc.CONSTRAINT_TABLE = 'SALES_HOT'"), "got: {out}");
        assert!(out.contains("AS \"column_name\"") && out.contains("AS \"to_table\""));

        // Incoming keys filter on the referenced side and label the local column from_column.
        let incoming = outgoing
            .replace("n.nspname = 'TEST'", "nf.nspname = 'TEST'")
            .replace("t.relname = 'SALES_HOT'", "tf.relname = 'SALES_HOT'")
            .replace("a.attname AS column_name", "a.attname AS from_column");
        let out_in = translate_postgres_to_exasol(&incoming).unwrap();
        assert!(out_in.contains("cc.REFERENCED_SCHEMA = 'TEST'") && out_in.contains("cc.REFERENCED_TABLE = 'SALES_HOT'"), "got: {out_in}");
        assert!(out_in.contains("AS \"from_column\""), "got: {out_in}");
    }

    #[test]
    fn rewrites_beekeeper_primary_keys_query() {
        // pg_index/indisprimary + '<table>'::regclass -> native constraint lookup.
        let sql = "SELECT a.attname as column_name, format_type(a.atttypid, a.atttypmod) AS data_type, \
                   a.attnum as position FROM pg_index i JOIN pg_attribute a \
                   ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
                   WHERE i.indrelid = '\"TEST\".\"SALES_HOT\"'::regclass AND i.indisprimary ORDER BY a.attnum";
        let out = translate_postgres_to_exasol(sql).unwrap();
        let upper = out.to_ascii_uppercase();
        assert!(!upper.contains("PG_INDEX"), "got: {out}");
        assert!(!upper.contains("REGCLASS"), "got: {out}");
        assert!(upper.contains("EXA_ALL_CONSTRAINT_COLUMNS"), "got: {out}");
        assert!(out.contains("'PRIMARY KEY'"));
        assert!(out.contains("'TEST'") && out.contains("'SALES_HOT'"), "got: {out}");
        assert!(out.contains("AS \"column_name\"") && out.contains("AS \"position\""));
    }

    #[test]
    fn quotes_position_and_condition_reserved_aliases() {
        // Beekeeper's getPrimaryKeys uses `AS position`; listTableTriggers uses
        // `AS condition`. Both are reserved in Exasol and must be quoted.
        assert!(
            translate_postgres_to_exasol("SELECT a.attnum AS position FROM t")
                .unwrap()
                .contains("AS \"position\"")
        );
        assert!(
            translate_postgres_to_exasol("SELECT action_condition AS condition FROM t")
                .unwrap()
                .contains("AS \"condition\"")
        );
    }

    #[test]
    fn neutralizes_beekeeper_col_description_format_lookup() {
        // Beekeeper's listTableColumns column-comment expression uses format()
        // and ::regclass, neither of which Exasol supports; collapsing it to
        // NULL keeps the column list working.
        let sql = "SELECT column_name, \
                   pg_catalog.col_description(format('%I.%I', table_schema, table_name)::regclass::oid, ordinal_position) AS column_comment \
                   FROM information_schema.columns WHERE table_schema = 'S' AND table_name = 'T'";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        let upper = translated.to_ascii_uppercase();
        assert!(!upper.contains("FORMAT("), "got: {translated}");
        assert!(!upper.contains("COL_DESCRIPTION"), "got: {translated}");
        assert!(!upper.contains("REGCLASS"), "got: {translated}");
        assert!(translated.contains("AS column_comment") || translated.contains("AS \"column_comment\""));
    }

    #[test]
    fn rewrites_beekeeper_table_list_with_inheritance_query() {
        // Beekeeper Studio sends this on connect to populate its table tree.
        // The `relnamespace::regnamespace::text` cast becomes an invalid
        // `CAST(... AS REGNAMESPACE)` in Exasol, so the gateway must answer it
        // from a native catalog query instead.
        let sql = "SELECT
        t.table_schema as schema,
        t.table_name as name,
          pc.relkind as tabletype,
          parent_pc.relkind as parenttype
        FROM information_schema.tables AS t
        JOIN pg_class AS pc
          ON t.table_name = pc.relname AND quote_ident(t.table_schema) = pc.relnamespace::regnamespace::text
        LEFT OUTER JOIN pg_inherits AS i
          ON pc.oid = i.inhrelid
        LEFT OUTER JOIN pg_class AS parent_pc
          ON parent_pc.oid = i.inhparent
        WHERE t.table_type NOT LIKE '%VIEW%'
      ORDER BY t.table_schema, t.table_name";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        let upper = translated.to_ascii_uppercase();
        // No invalid REGNAMESPACE cast survives.
        assert!(!upper.contains("REGNAMESPACE"), "got: {translated}");
        // Answered from the Exasol catalog with the columns Beekeeper expects.
        assert!(upper.contains("FROM SYS.EXA_ALL_TABLES"), "got: {translated}");
        assert!(translated.contains("AS \"schema\""));
        assert!(translated.contains("AS \"name\""));
        assert!(translated.contains("AS \"tabletype\""));
        assert!(translated.contains("AS \"parenttype\""));

        // The match is on the structural signature (tables view + pg_inherits +
        // relkind), not the client's exact aliases, so a variant that renames
        // the aliases and table-correlation names still resolves.
        let variant = "SELECT c.table_schema AS s, c.table_name AS n,
              k.relkind AS kind, p.relkind AS parentkind
            FROM information_schema.tables AS c
            JOIN pg_class AS k ON c.table_name = k.relname
              AND quote_ident(c.table_schema) = k.relnamespace::regnamespace::text
            LEFT OUTER JOIN pg_inherits AS h ON k.oid = h.inhrelid
            LEFT OUTER JOIN pg_class AS p ON p.oid = h.inhparent
            WHERE c.table_type NOT LIKE '%VIEW%'";
        let translated_variant = translate_postgres_to_exasol(variant).unwrap();
        assert!(!translated_variant.to_ascii_uppercase().contains("REGNAMESPACE"));
        assert!(
            translated_variant
                .to_ascii_uppercase()
                .contains("FROM SYS.EXA_ALL_TABLES"),
            "got: {translated_variant}"
        );
    }

    #[test]
    fn drops_correlated_scalar_subselect_in_beekeeper_pg_type_query() {
        // Beekeeper Studio's type-list query embeds a scalar correlated
        // subselect used as a boolean: `(SELECT c.relkind = 'c' FROM
        // pg_class c WHERE c.oid = t.typrelid)`. Exasol rejects this kind of
        // correlation. The rewrite collapses the whole disjunction to just
        // `(t.typrelid = 0)` — see the comment in `rewrite_sqlglot_edge_cases`.
        let sql = "SELECT n.nspname AS \"schema\", t.typname AS typename, \
                   CAST(t.oid AS INT) AS typeid \
                   FROM PG_CATALOG.PG_TYPE t \
                   LEFT JOIN PG_CATALOG.pg_namespace n ON n.oid = t.typnamespace \
                   WHERE (t.typrelid = 0 OR (SELECT c.relkind = 'c' FROM PG_CATALOG.pg_class c WHERE c.oid = t.typrelid)) \
                   AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
                   AND NOT EXISTS(SELECT 1 FROM PG_CATALOG.pg_type el WHERE el.oid = t.typelem AND el.typarray = t.oid)";
        let translated = translate_postgres_to_exasol(sql).unwrap();
        let lower = translated.to_ascii_lowercase();
        assert!(
            !lower.contains("c.relkind = 'c'"),
            "scalar correlated subselect should have been removed; got: {translated}"
        );
        assert!(lower.contains("t.typrelid = 0"));
    }
}
