use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementPlan {
    Execute {
        command: &'static str,
        row_count: RowCountPolicy,
    },
    ClientSet,
    SetSearchPath {
        schema: String,
    },
    ShowSearchPath,
    ClientTransactionStart,
    ClientTransactionEnd {
        command: &'static str,
    },
    ClientShow {
        name: String,
        value: String,
    },
    ClientSelect {
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
    },
    /// `SELECT pg_backend_pid()` — answered with the connection's own
    /// BackendKeyData pid, resolved in the execution path where the client
    /// (and thus its pid) is in scope. `column` carries the client's alias.
    BackendPid {
        column: String,
    },
    Cursor(CursorPlan),
    Empty,
    Reject {
        sqlstate: &'static str,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorPlan {
    Declare(CursorDeclare),
    Fetch(CursorPosition),
    Move(CursorPosition),
    Close(CursorClose),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorDeclare {
    pub name: String,
    pub query: String,
    pub scroll: bool,
    pub hold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorPosition {
    pub name: String,
    pub direction: CursorDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorClose {
    One(String),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDirection {
    Next,
    Prior,
    First,
    Last,
    Absolute(i64),
    Relative(i64),
    Forward(Option<i64>),
    Backward(Option<i64>),
    All,
    Count(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowCountPolicy {
    Include,
    Omit,
}

impl StatementPlan {
    pub fn is_cursor_unsafe_query(&self) -> bool {
        !matches!(
            self,
            StatementPlan::Execute {
                command: "SELECT",
                ..
            }
        )
    }
}

static COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/|--[^\n\r]*").unwrap());
// Accepts both `SET key = value` and `SET key=value` (no spaces around `=`),
// the latter being what pgAdmin emits for `SET DateStyle=ISO`. The trailing
// `\S` keeps `SET key=` (missing value) from sneaking through.
static SET_ASSIGN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^SET\s+(SESSION\s+)?[A-Za-z_][A-Za-z0-9_.]*\s*(=|TO)\s*\S").unwrap()
});
static RESET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^RESET\s+(ALL|[A-Za-z_][A-Za-z0-9_.]*)$").unwrap());
static SHOW_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^SHOW\s+([A-Za-z_][A-Za-z0-9_ .-]*)$").unwrap());
static SET_SEARCH_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^SET\s+(?:SESSION\s+)?search_path\s*(?:=|TO\b)\s*(.+)$").unwrap()
});
static DECLARE_CURSOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)^DECLARE\s+(?P<name>"[^"]+"|[A-Za-z_][A-Za-z0-9_]*)\s+(?P<options>(?:(?:BINARY|INSENSITIVE|ASENSITIVE|NO\s+SCROLL|SCROLL)\s+)*)CURSOR\s*(?P<hold>WITH\s+HOLD|WITHOUT\s+HOLD)?\s+FOR\s+(?P<query>.+)$"#,
    )
    .unwrap()
});

pub fn classify_statement(sql: &str) -> StatementPlan {
    let cleaned = normalize_sql(sql);
    if cleaned.is_empty() {
        return StatementPlan::Empty;
    }

    if let Some(cap) = SET_SEARCH_PATH_RE.captures(&cleaned) {
        let rhs = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        return match parse_search_path_value(rhs) {
            SearchPathTarget::Single(schema) => StatementPlan::SetSearchPath { schema },
            SearchPathTarget::Default => StatementPlan::ClientSet,
            SearchPathTarget::Invalid => StatementPlan::Reject {
                sqlstate: "42601",
                message: "invalid search_path value".to_owned(),
            },
        };
    }

    if is_safe_set(&cleaned) || RESET_RE.is_match(&cleaned) {
        return StatementPlan::ClientSet;
    }

    if let Some(show) = local_show(&cleaned) {
        return show;
    }

    if let Some(select) = local_select(&cleaned) {
        return select;
    }

    let keyword = first_keyword(&cleaned);
    match keyword.as_str() {
        "SELECT" | "WITH" | "VALUES" => execute("SELECT", RowCountPolicy::Omit),
        "INSERT" => execute("INSERT", RowCountPolicy::Include),
        "UPDATE" => execute("UPDATE", RowCountPolicy::Include),
        "DELETE" => execute("DELETE", RowCountPolicy::Include),
        "MERGE" => execute("MERGE", RowCountPolicy::Include),
        "TRUNCATE" => execute("TRUNCATE TABLE", RowCountPolicy::Omit),
        "CREATE" => classify_create(&cleaned),
        "ALTER" => classify_alter(&cleaned),
        "DROP" => classify_drop(&cleaned),
        "COMMENT" => execute("COMMENT", RowCountPolicy::Omit),
        "GRANT" => execute("GRANT", RowCountPolicy::Omit),
        "REVOKE" => execute("REVOKE", RowCountPolicy::Omit),
        "COPY" => unsupported_policy(
            "COPY requires a separate bulk data movement design before it can be exposed",
        ),
        "IMPORT" | "EXPORT" => unsupported_policy(
            "Exasol IMPORT/EXPORT are not PostgreSQL commands and are not exposed through the gateway policy",
        ),
        "DECLARE" => classify_declare_cursor(&cleaned),
        "FETCH" => classify_cursor_position("FETCH", &cleaned),
        "MOVE" => classify_cursor_position("MOVE", &cleaned),
        "CLOSE" => classify_close_cursor(&cleaned),
        "PREPARE" | "EXECUTE" | "DEALLOCATE" => unsupported_policy(
            "SQL prepared statement commands are not implemented; use PostgreSQL extended query protocol",
        ),
        "BEGIN" | "START" => StatementPlan::ClientTransactionStart,
        "COMMIT" if second_keyword(&cleaned) == "PREPARED" => unsupported_no_equivalent(
            "PostgreSQL two-phase commit is not supported because no safe Exasol equivalent is configured",
        ),
        "COMMIT" => StatementPlan::ClientTransactionEnd { command: "COMMIT" },
        "ROLLBACK" if matches!(second_keyword(&cleaned).as_str(), "TO" | "PREPARED") => {
            unsupported_no_equivalent(
                "PostgreSQL savepoints and two-phase commit are not supported because no safe Exasol equivalent is configured",
            )
        }
        "ROLLBACK" => StatementPlan::ClientTransactionEnd {
            command: "ROLLBACK",
        },
        "SAVEPOINT" | "RELEASE" => unsupported_no_equivalent(
            "PostgreSQL savepoints are not supported because no safe Exasol equivalent is configured",
        ),
        "ABORT" => StatementPlan::ClientTransactionEnd {
            command: "ROLLBACK",
        },
        "END" => StatementPlan::ClientTransactionEnd { command: "COMMIT" },
        "SET" | "RESET" | "SHOW" => StatementPlan::Reject {
            sqlstate: "0A000",
            message: "unsupported PostgreSQL session command".to_owned(),
        },
        "LISTEN" | "NOTIFY" | "UNLISTEN" => unsupported_no_equivalent(
            "PostgreSQL asynchronous notification commands have no Exasol equivalent",
        ),
        "VACUUM" | "REINDEX" | "CLUSTER" | "CHECKPOINT" | "ANALYZE" => unsupported_no_equivalent(
            "PostgreSQL maintenance commands have no safe Exasol equivalent in this gateway",
        ),
        other => StatementPlan::Reject {
            sqlstate: "0A000",
            message: format!("unsupported SQL statement class: {other}"),
        },
    }
}

fn classify_declare_cursor(sql: &str) -> StatementPlan {
    let Some(cap) = DECLARE_CURSOR_RE.captures(sql) else {
        return StatementPlan::Reject {
            sqlstate: "42601",
            message: "unsupported DECLARE syntax; only SQL cursor declarations are supported"
                .to_owned(),
        };
    };
    let name = unquote_identifier(cap.name("name").map(|m| m.as_str()).unwrap_or_default());
    let options = cap
        .name("options")
        .map(|m| m.as_str())
        .unwrap_or_default()
        .to_ascii_uppercase();
    let hold = cap
        .name("hold")
        .is_some_and(|m| m.as_str().eq_ignore_ascii_case("WITH HOLD"));
    let query = cap
        .name("query")
        .map(|m| m.as_str().trim().to_owned())
        .unwrap_or_default();

    if options.split_whitespace().any(|token| token == "BINARY") {
        return unsupported_policy("binary SQL cursors are not implemented");
    }
    if options.contains("INSENSITIVE") || options.contains("ASENSITIVE") {
        return unsupported_policy("cursor sensitivity options are not implemented");
    }
    if contains_positioned_cursor_write(&query) {
        return unsupported_policy(
            "updatable cursors, FOR UPDATE, FOR SHARE, and positioned writes are not implemented",
        );
    }

    StatementPlan::Cursor(CursorPlan::Declare(CursorDeclare {
        name,
        query,
        scroll: options.contains("SCROLL") && !options.contains("NO SCROLL"),
        hold,
    }))
}

fn classify_cursor_position(command: &'static str, sql: &str) -> StatementPlan {
    let Some(position) = parse_cursor_position(command, sql) else {
        return StatementPlan::Reject {
            sqlstate: "42601",
            message: format!("unsupported {command} cursor syntax"),
        };
    };
    match command {
        "FETCH" => StatementPlan::Cursor(CursorPlan::Fetch(position)),
        "MOVE" => StatementPlan::Cursor(CursorPlan::Move(position)),
        _ => unreachable!("unsupported cursor position command"),
    }
}

fn classify_close_cursor(sql: &str) -> StatementPlan {
    let tokens = sql.split_whitespace().collect::<Vec<_>>();
    if tokens.len() != 2 {
        return StatementPlan::Reject {
            sqlstate: "42601",
            message: "unsupported CLOSE cursor syntax".to_owned(),
        };
    }
    if tokens[1].eq_ignore_ascii_case("ALL") {
        StatementPlan::Cursor(CursorPlan::Close(CursorClose::All))
    } else {
        StatementPlan::Cursor(CursorPlan::Close(CursorClose::One(unquote_identifier(
            tokens[1],
        ))))
    }
}

fn parse_cursor_position(command: &str, sql: &str) -> Option<CursorPosition> {
    let tokens = sql.split_whitespace().collect::<Vec<_>>();
    if tokens.first()?.to_ascii_uppercase() != command || tokens.len() < 2 {
        return None;
    }

    let (direction_tokens, name) = if tokens.len() >= 3
        && matches!(
            tokens[tokens.len() - 2].to_ascii_uppercase().as_str(),
            "FROM" | "IN"
        ) {
        (&tokens[1..tokens.len() - 2], tokens[tokens.len() - 1])
    } else if tokens.len() == 2 {
        (&[][..], tokens[1])
    } else {
        (&tokens[1..tokens.len() - 1], tokens[tokens.len() - 1])
    };

    Some(CursorPosition {
        name: unquote_identifier(name),
        direction: parse_cursor_direction(direction_tokens)?,
    })
}

fn parse_cursor_direction(tokens: &[&str]) -> Option<CursorDirection> {
    match tokens {
        [] => Some(CursorDirection::Next),
        [one] if one.eq_ignore_ascii_case("NEXT") => Some(CursorDirection::Next),
        [one] if one.eq_ignore_ascii_case("PRIOR") => Some(CursorDirection::Prior),
        [one] if one.eq_ignore_ascii_case("FIRST") => Some(CursorDirection::First),
        [one] if one.eq_ignore_ascii_case("LAST") => Some(CursorDirection::Last),
        [one] if one.eq_ignore_ascii_case("ALL") => Some(CursorDirection::All),
        [one] if one.eq_ignore_ascii_case("FORWARD") => Some(CursorDirection::Forward(Some(1))),
        [one] if one.eq_ignore_ascii_case("BACKWARD") => Some(CursorDirection::Backward(Some(1))),
        [one] => one.parse::<i64>().ok().map(CursorDirection::Count),
        [keyword, value] if keyword.eq_ignore_ascii_case("ABSOLUTE") => {
            value.parse::<i64>().ok().map(CursorDirection::Absolute)
        }
        [keyword, value] if keyword.eq_ignore_ascii_case("RELATIVE") => {
            value.parse::<i64>().ok().map(CursorDirection::Relative)
        }
        [keyword, value] if keyword.eq_ignore_ascii_case("FORWARD") => {
            if value.eq_ignore_ascii_case("ALL") {
                Some(CursorDirection::Forward(None))
            } else {
                value.parse::<i64>().ok().map(|count| {
                    if count < 0 {
                        CursorDirection::Backward(Some(-count))
                    } else {
                        CursorDirection::Forward(Some(count))
                    }
                })
            }
        }
        [keyword, value] if keyword.eq_ignore_ascii_case("BACKWARD") => {
            if value.eq_ignore_ascii_case("ALL") {
                Some(CursorDirection::Backward(None))
            } else {
                value.parse::<i64>().ok().map(|count| {
                    if count < 0 {
                        CursorDirection::Forward(Some(-count))
                    } else {
                        CursorDirection::Backward(Some(count))
                    }
                })
            }
        }
        _ => None,
    }
}

fn contains_positioned_cursor_write(query: &str) -> bool {
    Regex::new(
        r"(?i)\bFOR\s+(UPDATE|SHARE|NO\s+KEY\s+UPDATE|KEY\s+SHARE)\b|\bWHERE\s+CURRENT\s+OF\b",
    )
    .unwrap()
    .is_match(query)
}

fn unquote_identifier(identifier: &str) -> String {
    identifier
        .trim()
        .trim_end_matches(';')
        .trim_matches('"')
        .replace("\"\"", "\"")
}

fn execute(command: &'static str, row_count: RowCountPolicy) -> StatementPlan {
    StatementPlan::Execute { command, row_count }
}

fn classify_create(sql: &str) -> StatementPlan {
    match second_keyword(sql).as_str() {
        "TABLE" => execute("CREATE TABLE", RowCountPolicy::Omit),
        "VIEW" => execute("CREATE VIEW", RowCountPolicy::Omit),
        "SCHEMA" => execute("CREATE SCHEMA", RowCountPolicy::Omit),
        "ROLE" | "USER" => unsupported_policy(
            "role and user creation require explicit PostgreSQL-to-Exasol privilege mapping before they can be exposed",
        ),
        "FUNCTION" | "PROCEDURE" | "SCRIPT" => unsupported_policy(
            "routine creation requires explicit PostgreSQL-to-Exasol routine mapping before it can be exposed",
        ),
        "INDEX" | "EXTENSION" | "PUBLICATION" | "SUBSCRIPTION" | "POLICY" | "RULE" | "EVENT"
        | "TRIGGER" | "TABLESPACE" | "SERVER" | "FOREIGN" | "ACCESS" | "TEXT" | "OPERATOR"
        | "LANGUAGE" | "CAST" | "COLLATION" | "CONVERSION" | "DOMAIN" | "TYPE" | "STATISTICS"
        | "TRANSFORM" => unsupported_no_equivalent(&format!(
            "PostgreSQL CREATE {} has no supported Exasol equivalent in this gateway",
            second_keyword(sql)
        )),
        other => StatementPlan::Reject {
            sqlstate: "0A000",
            message: format!("unsupported PostgreSQL CREATE target: {other}"),
        },
    }
}

fn classify_alter(sql: &str) -> StatementPlan {
    match second_keyword(sql).as_str() {
        "TABLE" => execute("ALTER TABLE", RowCountPolicy::Omit),
        "VIEW" => execute("ALTER VIEW", RowCountPolicy::Omit),
        "SCHEMA" => execute("ALTER SCHEMA", RowCountPolicy::Omit),
        "ROLE" | "USER" => unsupported_policy(
            "role and user alteration require explicit PostgreSQL-to-Exasol privilege mapping before they can be exposed",
        ),
        "SYSTEM" => {
            unsupported_policy("ALTER SYSTEM is not exposed through the PostgreSQL gateway policy")
        }
        "FUNCTION" | "PROCEDURE" | "ROUTINE" | "EXTENSION" | "PUBLICATION" | "SUBSCRIPTION"
        | "POLICY" | "RULE" | "EVENT" | "TRIGGER" | "TABLESPACE" | "SERVER" | "FOREIGN"
        | "ACCESS" | "TEXT" | "OPERATOR" | "LANGUAGE" | "COLLATION" | "CONVERSION" | "DOMAIN"
        | "TYPE" | "STATISTICS" => unsupported_no_equivalent(&format!(
            "PostgreSQL ALTER {} has no supported Exasol equivalent in this gateway",
            second_keyword(sql)
        )),
        other => StatementPlan::Reject {
            sqlstate: "0A000",
            message: format!("unsupported PostgreSQL ALTER target: {other}"),
        },
    }
}

fn classify_drop(sql: &str) -> StatementPlan {
    match second_keyword(sql).as_str() {
        "TABLE" => execute("DROP TABLE", RowCountPolicy::Omit),
        "VIEW" => execute("DROP VIEW", RowCountPolicy::Omit),
        "SCHEMA" => execute("DROP SCHEMA", RowCountPolicy::Omit),
        "ROLE" | "USER" => unsupported_policy(
            "role and user dropping require explicit PostgreSQL-to-Exasol privilege mapping before they can be exposed",
        ),
        "FUNCTION" | "PROCEDURE" | "SCRIPT" => unsupported_policy(
            "routine dropping requires explicit PostgreSQL-to-Exasol routine mapping before it can be exposed",
        ),
        "INDEX" | "EXTENSION" | "PUBLICATION" | "SUBSCRIPTION" | "POLICY" | "RULE" | "EVENT"
        | "TRIGGER" | "TABLESPACE" | "SERVER" | "FOREIGN" | "ACCESS" | "TEXT" | "OPERATOR"
        | "LANGUAGE" | "CAST" | "COLLATION" | "CONVERSION" | "DOMAIN" | "TYPE" | "STATISTICS"
        | "TRANSFORM" | "OWNED" => unsupported_no_equivalent(&format!(
            "PostgreSQL DROP {} has no supported Exasol equivalent in this gateway",
            second_keyword(sql)
        )),
        other => StatementPlan::Reject {
            sqlstate: "0A000",
            message: format!("unsupported PostgreSQL DROP target: {other}"),
        },
    }
}

fn unsupported_policy(message: &str) -> StatementPlan {
    StatementPlan::Reject {
        sqlstate: "0A000",
        message: format!("unsupported by gateway policy: {message}"),
    }
}

fn unsupported_no_equivalent(message: &str) -> StatementPlan {
    StatementPlan::Reject {
        sqlstate: "0A000",
        message: format!("unsupported because no Exasol equivalent is available: {message}"),
    }
}

fn normalize_sql(sql: &str) -> String {
    COMMENT_RE
        .replace_all(sql, " ")
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_owned()
}

fn first_keyword(sql: &str) -> String {
    sql.split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_start_matches('(')
        .to_ascii_uppercase()
}

fn second_keyword(sql: &str) -> String {
    let mut tokens = sql.split_whitespace().skip(1).map(|token| {
        token
            .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .to_ascii_uppercase()
    });
    let first = tokens.next().unwrap_or_default();
    if first == "OR" && tokens.next().as_deref() == Some("REPLACE") {
        tokens.next().unwrap_or_default()
    } else {
        first
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SearchPathTarget {
    Single(String),
    Default,
    Invalid,
}

fn parse_search_path_value(rhs: &str) -> SearchPathTarget {
    let trimmed = rhs.trim();
    if trimmed.is_empty() {
        return SearchPathTarget::Invalid;
    }

    // PostgreSQL allows a comma-separated list of schemas; Exasol's OPEN
    // SCHEMA only takes one. Silently keep the first entry and drop the rest
    // so client tools that emit `SET search_path = "a", "b"` still succeed.
    if let Some(idx) = top_level_comma_index(trimmed) {
        return parse_search_path_value(&trimmed[..idx]);
    }

    // Bare identifier `DEFAULT` (case-insensitive, unquoted) acts as a
    // reset to the server default and is treated as a generic ClientSet.
    if trimmed.eq_ignore_ascii_case("DEFAULT") {
        return SearchPathTarget::Default;
    }

    // Double-quoted identifier: strip enclosing quotes and unescape `""`.
    if let Some(inner) = strip_enclosing(trimmed, '"', '"') {
        let unescaped = inner.replace("\"\"", "\"");
        if unescaped.is_empty() {
            return SearchPathTarget::Invalid;
        }
        return SearchPathTarget::Single(unescaped);
    }

    // Single-quoted string literal: strip enclosing quotes.
    if let Some(inner) = strip_enclosing(trimmed, '\'', '\'') {
        if inner.is_empty() {
            return SearchPathTarget::Invalid;
        }
        return SearchPathTarget::Single(inner.to_owned());
    }

    // Bare identifier: take it as-is.
    SearchPathTarget::Single(trimmed.to_owned())
}

fn top_level_comma_index(value: &str) -> Option<usize> {
    let mut in_double = false;
    let mut in_single = false;
    let mut chars = value.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '"' if !in_single => {
                // Handle `""` escape inside a double-quoted segment by
                // consuming the second quote so we stay inside the segment.
                if in_double && chars.peek().map(|(_, c)| *c) == Some('"') {
                    chars.next();
                } else {
                    in_double = !in_double;
                }
            }
            '\'' if !in_double => {
                if in_single && chars.peek().map(|(_, c)| *c) == Some('\'') {
                    chars.next();
                } else {
                    in_single = !in_single;
                }
            }
            ',' if !in_double && !in_single => return Some(idx),
            _ => {}
        }
    }
    None
}

fn strip_enclosing(value: &str, open: char, close: char) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let first = value.chars().next()?;
    let last = value.chars().next_back()?;
    if first != open || last != close {
        return None;
    }
    let start = first.len_utf8();
    let end = value.len() - last.len_utf8();
    Some(&value[start..end])
}

fn is_safe_set(sql: &str) -> bool {
    if Regex::new(r"(?i)^SET\s+SESSION\s+CHARACTERISTICS\s+AS\s+TRANSACTION\b")
        .unwrap()
        .is_match(sql)
    {
        return true;
    }
    if Regex::new(r"(?i)^SET\s+(SESSION\s+)?TRANSACTION\b")
        .unwrap()
        .is_match(sql)
    {
        return false;
    }
    SET_ASSIGN_RE.is_match(sql)
}

fn local_show(sql: &str) -> Option<StatementPlan> {
    let cap = SHOW_RE.captures(sql)?;
    let raw_name = cap.get(1)?.as_str().trim();
    let key = raw_name.replace([' ', '-'], "_").to_ascii_lowercase();
    if key == "search_path" {
        return Some(StatementPlan::ShowSearchPath);
    }
    let value = match key.as_str() {
        "datestyle" => "ISO, YMD",
        "timezone" | "time_zone" => "Etc/UTC",
        "transaction_isolation" | "transaction_isolation_level" => "read committed",
        "transaction_read_only" => "off",
        "standard_conforming_strings" => "on",
        "client_encoding" => "UTF8",
        "server_version" => "16.6-exasol-gateway",
        "application_name" => "",
        _ => return None,
    };
    Some(StatementPlan::ClientShow {
        name: raw_name.to_owned(),
        value: value.to_owned(),
    })
}

fn local_select(sql: &str) -> Option<StatementPlan> {
    let lower = sql.to_ascii_lowercase();
    if lower.contains("pg_catalog.pg_settings") || lower.contains(" pg_settings") {
        return Some(catalog_pg_settings(sql));
    }
    if lower == "select version()" {
        return Some(single_value(
            "version",
            "PostgreSQL 16.6 compatible Exasol gateway",
        ));
    }
    if lower == "select current_database()" {
        return Some(single_value("current_database", "exasol"));
    }
    if lower == "select current_catalog" || lower == "select current_catalog()" {
        return Some(single_value("current_catalog", "exasol"));
    }
    if lower == "select current_user" || lower == "select user" {
        return Some(single_value("current_user", "sys"));
    }
    // Beekeeper (and other clients) call pg_backend_pid() to label the
    // connection. Exasol has no such function, so it errors if the call
    // reaches the engine. Answer it with the connection's own BackendKeyData
    // pid (resolved later, where the client is in scope) and honor the
    // client's alias when present (e.g. `... AS pid`).
    if lower == "select pg_backend_pid()" || lower.starts_with("select pg_backend_pid() as ") {
        // No FROM clause here, so select_alias() can't help; take the alias
        // straight from the matched `... AS <name>` suffix.
        let column = lower
            .find(" as ")
            .map(|idx| unquote_identifier(&sql[idx + 4..]))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "pg_backend_pid".to_owned());
        return Some(StatementPlan::BackendPid { column });
    }
    None
}

fn single_value(name: &str, value: &str) -> StatementPlan {
    StatementPlan::ClientSelect {
        columns: vec![name.to_owned()],
        rows: vec![vec![Some(value.to_owned())]],
    }
}

fn catalog_pg_settings(sql: &str) -> StatementPlan {
    catalog_response_many(
        sql,
        &[
            vec![
                ("name", "server_version"),
                ("setting", "16.6-exasol-gateway"),
            ],
            vec![("name", "client_encoding"), ("setting", "UTF8")],
            vec![("name", "standard_conforming_strings"), ("setting", "on")],
            vec![("name", "TimeZone"), ("setting", "Etc/UTC")],
        ],
    )
}

fn catalog_response_many(sql: &str, source_rows: &[Vec<(&str, &str)>]) -> StatementPlan {
    let lower = sql.to_ascii_lowercase();
    if lower.contains("count(") {
        return StatementPlan::ClientSelect {
            columns: vec![select_alias(sql).unwrap_or_else(|| "count".to_owned())],
            rows: vec![vec![Some(source_rows.len().to_string())]],
        };
    }

    let projections =
        catalog_projection(sql, source_rows.first().map(Vec::as_slice).unwrap_or(&[]));
    let columns = projections
        .iter()
        .map(|projection| projection.output_name.clone())
        .collect::<Vec<_>>();
    let rows = source_rows
        .iter()
        .map(|row| {
            projections
                .iter()
                .map(|projection| {
                    row.iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case(&projection.source_name))
                        .map(|(_, value)| (*value).to_owned())
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    StatementPlan::ClientSelect { columns, rows }
}

#[derive(Debug)]
struct CatalogProjection {
    source_name: String,
    output_name: String,
}

fn catalog_projection(sql: &str, default_row: &[(&str, &str)]) -> Vec<CatalogProjection> {
    let Some(select_list) = select_list(sql) else {
        return default_projection(default_row);
    };
    if select_list.trim() == "*" || select_list.trim().ends_with(".*") {
        return default_projection(default_row);
    }

    split_select_items(select_list)
        .into_iter()
        .map(|item| projection_from_item(&item))
        .collect()
}

fn default_projection(default_row: &[(&str, &str)]) -> Vec<CatalogProjection> {
    default_row
        .iter()
        .map(|(name, _)| CatalogProjection {
            source_name: (*name).to_owned(),
            output_name: (*name).to_owned(),
        })
        .collect()
}

fn select_list(sql: &str) -> Option<&str> {
    let lower = sql.to_ascii_lowercase();
    let select_start = lower.find("select")? + "select".len();
    let from_start = lower[select_start..].find(" from ")? + select_start;
    let mut list = sql[select_start..from_start].trim();
    if list.to_ascii_lowercase().starts_with("distinct ") {
        list = list[8..].trim();
    }
    Some(list)
}

fn select_alias(sql: &str) -> Option<String> {
    split_select_items(select_list(sql)?)
        .into_iter()
        .next()
        .map(|item| projection_from_item(&item).output_name)
}

fn split_select_items(select_list: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_single_quote = false;
    for (idx, ch) in select_list.char_indices() {
        match ch {
            '\'' => in_single_quote = !in_single_quote,
            '(' if !in_single_quote => depth += 1,
            ')' if !in_single_quote => depth -= 1,
            ',' if !in_single_quote && depth == 0 => {
                items.push(select_list[start..idx].trim().to_owned());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = select_list[start..].trim();
    if !tail.is_empty() {
        items.push(tail.to_owned());
    }
    items
}

fn projection_from_item(item: &str) -> CatalogProjection {
    let (expr, alias) = split_alias(item);
    let source_name = source_column_name(expr);
    CatalogProjection {
        source_name: source_name.clone(),
        output_name: alias.unwrap_or(source_name),
    }
}

fn split_alias(item: &str) -> (&str, Option<String>) {
    static AS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)^(.*?)\s+AS\s+"?([A-Za-z_][A-Za-z0-9_]*)"?$"#).unwrap());
    if let Some(cap) = AS_RE.captures(item) {
        return (
            cap.get(1).map(|m| m.as_str()).unwrap_or(item).trim(),
            cap.get(2).map(|m| m.as_str().to_owned()),
        );
    }
    (item, None)
}

fn source_column_name(expr: &str) -> String {
    let expr = expr.trim().trim_matches('"');
    let last = expr
        .rsplit('.')
        .next()
        .unwrap_or(expr)
        .trim()
        .trim_matches('"')
        .to_ascii_lowercase();
    if last.contains("datname") {
        "datname".to_owned()
    } else if last.contains("nspname") {
        "nspname".to_owned()
    } else if last.contains("rolname") {
        "rolname".to_owned()
    } else if last.contains("current_database") || last.contains("current_catalog") {
        "datname".to_owned()
    } else {
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_read_and_write() {
        assert_eq!(
            classify_statement("SELECT 1"),
            StatementPlan::Execute {
                command: "SELECT",
                row_count: RowCountPolicy::Omit
            }
        );
        assert_eq!(
            classify_statement("DELETE FROM t"),
            StatementPlan::Execute {
                command: "DELETE",
                row_count: RowCountPolicy::Include
            }
        );
        assert_eq!(
            classify_statement("INSERT INTO t VALUES (1)"),
            StatementPlan::Execute {
                command: "INSERT",
                row_count: RowCountPolicy::Include
            }
        );
    }

    #[test]
    fn classifies_supported_ddl_by_object_type() {
        assert_eq!(
            classify_statement("CREATE TABLE t (id int)"),
            StatementPlan::Execute {
                command: "CREATE TABLE",
                row_count: RowCountPolicy::Omit
            }
        );
        assert_eq!(
            classify_statement("DROP VIEW v"),
            StatementPlan::Execute {
                command: "DROP VIEW",
                row_count: RowCountPolicy::Omit
            }
        );
        assert_eq!(
            classify_statement("CREATE OR REPLACE TABLE t AS SELECT 1 AS id"),
            StatementPlan::Execute {
                command: "CREATE TABLE",
                row_count: RowCountPolicy::Omit
            }
        );
    }

    #[test]
    fn rejects_postgres_only_objects_with_no_equivalent() {
        assert!(matches!(
            classify_statement("CREATE EXTENSION hstore"),
            StatementPlan::Reject { message, .. } if message.contains("no Exasol equivalent")
        ));
        assert!(matches!(
            classify_statement("LISTEN channel"),
            StatementPlan::Reject { message, .. } if message.contains("no Exasol equivalent")
        ));
    }

    #[test]
    fn rejects_unimplemented_gateway_managed_capabilities_by_policy() {
        assert!(matches!(
            classify_statement("CREATE USER u IDENTIFIED BY 'secret'"),
            StatementPlan::Reject { message, .. } if message.contains("unsupported by gateway policy")
        ));
    }

    #[test]
    fn classifies_supported_cursor_commands() {
        assert_eq!(
            classify_statement("DECLARE plain_cursor CURSOR FOR SELECT 1"),
            StatementPlan::Cursor(CursorPlan::Declare(CursorDeclare {
                name: "plain_cursor".to_owned(),
                query: "SELECT 1".to_owned(),
                scroll: false,
                hold: false,
            }))
        );
        assert_eq!(
            classify_statement("DECLARE c SCROLL CURSOR WITH HOLD FOR SELECT 1"),
            StatementPlan::Cursor(CursorPlan::Declare(CursorDeclare {
                name: "c".to_owned(),
                query: "SELECT 1".to_owned(),
                scroll: true,
                hold: true,
            }))
        );
        assert_eq!(
            classify_statement("FETCH FORWARD 5 FROM c"),
            StatementPlan::Cursor(CursorPlan::Fetch(CursorPosition {
                name: "c".to_owned(),
                direction: CursorDirection::Forward(Some(5)),
            }))
        );
        assert_eq!(
            classify_statement("FETCH FORWARD FROM c"),
            StatementPlan::Cursor(CursorPlan::Fetch(CursorPosition {
                name: "c".to_owned(),
                direction: CursorDirection::Forward(Some(1)),
            }))
        );
        assert_eq!(
            classify_statement("MOVE BACKWARD ALL IN c"),
            StatementPlan::Cursor(CursorPlan::Move(CursorPosition {
                name: "c".to_owned(),
                direction: CursorDirection::Backward(None),
            }))
        );
        assert_eq!(
            classify_statement("CLOSE c"),
            StatementPlan::Cursor(CursorPlan::Close(CursorClose::One("c".to_owned())))
        );
    }

    #[test]
    fn rejects_unsupported_cursor_semantics() {
        assert!(matches!(
            classify_statement("DECLARE c BINARY CURSOR FOR SELECT 1"),
            StatementPlan::Reject { message, .. } if message.contains("binary")
        ));
        assert!(matches!(
            classify_statement("DECLARE c CURSOR FOR SELECT * FROM t FOR UPDATE"),
            StatementPlan::Reject { message, .. } if message.contains("updatable cursors")
        ));
    }

    #[test]
    fn handles_driver_session_commands_locally() {
        assert_eq!(
            classify_statement("SET extra_float_digits = 3"),
            StatementPlan::ClientSet
        );
        assert_eq!(
            classify_statement("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"),
            StatementPlan::ClientSet
        );
        assert!(matches!(
            classify_statement("SHOW transaction isolation level"),
            StatementPlan::ClientShow { .. }
        ));
    }

    #[test]
    fn accepts_set_with_no_spaces_around_equals() {
        // pgAdmin sends `SET DateStyle=ISO` on every connection; spaced and
        // unspaced forms must both be swallowed locally.
        assert_eq!(
            classify_statement("SET DateStyle=ISO"),
            StatementPlan::ClientSet
        );
        assert_eq!(
            classify_statement("SET DateStyle = ISO"),
            StatementPlan::ClientSet
        );
        assert_eq!(
            classify_statement("SET client_min_messages='warning'"),
            StatementPlan::ClientSet
        );
        // Missing value still rejected.
        assert!(matches!(
            classify_statement("SET DateStyle="),
            StatementPlan::Reject { .. }
        ));
    }

    #[test]
    fn handles_driver_selects_locally() {
        assert!(matches!(
            classify_statement("SELECT version()"),
            StatementPlan::ClientSelect { .. }
        ));
    }

    #[test]
    fn answers_pg_backend_pid_locally() {
        // Exasol has no pg_backend_pid(); the gateway answers it from the
        // connection's own pid (resolved in the execution path) and preserves
        // the client's alias.
        assert_eq!(
            classify_statement("SELECT pg_backend_pid()"),
            StatementPlan::BackendPid {
                column: "pg_backend_pid".to_owned()
            }
        );
        assert_eq!(
            classify_statement("SELECT pg_backend_pid() AS pid"),
            StatementPlan::BackendPid {
                column: "pid".to_owned()
            }
        );
    }

    #[test]
    fn lets_pg_database_catalog_query_reach_exasol() {
        assert_eq!(
            classify_statement(
                "SELECT d.datname AS table_cat FROM pg_catalog.pg_database d ORDER BY d.datname",
            ),
            StatementPlan::Execute {
                command: "SELECT",
                row_count: RowCountPolicy::Omit
            }
        );
    }

    #[test]
    fn lets_pg_namespace_catalog_query_reach_exasol() {
        assert_eq!(
            classify_statement("SELECT n.nspname AS table_schem FROM pg_catalog.pg_namespace n"),
            StatementPlan::Execute {
                command: "SELECT",
                row_count: RowCountPolicy::Omit
            }
        );
    }

    #[test]
    fn classifies_set_search_path_single_schema() {
        assert_eq!(
            classify_statement(r#"SET search_path = "DEMO_FINANCE""#),
            StatementPlan::SetSearchPath {
                schema: "DEMO_FINANCE".to_owned()
            }
        );
        assert_eq!(
            classify_statement("SET search_path TO demo_finance"),
            StatementPlan::SetSearchPath {
                schema: "demo_finance".to_owned()
            }
        );
        assert_eq!(
            classify_statement("SET SESSION search_path = 'pg_demo'"),
            StatementPlan::SetSearchPath {
                schema: "pg_demo".to_owned()
            }
        );
    }

    #[test]
    fn set_search_path_multi_schema_keeps_first_entry() {
        // PostgreSQL clients commonly emit a list (e.g. `"DEMO","public"`).
        // Exasol has no equivalent, so silently keep the first schema and
        // drop the rest rather than rejecting the whole statement.
        assert_eq!(
            classify_statement("SET search_path = pg_demo, pg_catalog"),
            StatementPlan::SetSearchPath {
                schema: "pg_demo".to_owned()
            }
        );
        assert_eq!(
            classify_statement(r#"SET search_path TO "DEMO_SANDBOX","public""#),
            StatementPlan::SetSearchPath {
                schema: "DEMO_SANDBOX".to_owned()
            }
        );
    }

    #[test]
    fn handles_search_path_reset_and_default() {
        assert_eq!(
            classify_statement("SET search_path = DEFAULT"),
            StatementPlan::ClientSet
        );
        assert_eq!(
            classify_statement("RESET search_path"),
            StatementPlan::ClientSet
        );
    }

    #[test]
    fn classifies_show_search_path_dynamically() {
        assert_eq!(
            classify_statement("SHOW search_path"),
            StatementPlan::ShowSearchPath
        );
    }

    #[test]
    fn other_set_statements_still_classify_as_client_set() {
        assert_eq!(
            classify_statement("SET application_name = 'myapp'"),
            StatementPlan::ClientSet
        );
        assert_eq!(
            classify_statement("SET extra_float_digits = 3"),
            StatementPlan::ClientSet
        );
        assert_eq!(
            classify_statement("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"),
            StatementPlan::ClientSet
        );
    }

    #[test]
    fn handles_transaction_wrappers_locally() {
        assert_eq!(
            classify_statement("BEGIN"),
            StatementPlan::ClientTransactionStart
        );
        assert_eq!(
            classify_statement("COMMIT"),
            StatementPlan::ClientTransactionEnd { command: "COMMIT" }
        );
        assert!(matches!(
            classify_statement("ROLLBACK TO SAVEPOINT s1"),
            StatementPlan::Reject { message, .. } if message.contains("no Exasol equivalent")
        ));
        assert!(matches!(
            classify_statement("COMMIT PREPARED 'x'"),
            StatementPlan::Reject { message, .. } if message.contains("no Exasol equivalent")
        ));
    }
}
