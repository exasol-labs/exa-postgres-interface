use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::config::AppConfig;
use crate::exasol::{ExasolError, ExasolResult, ExasolSession};

const DEFAULT_CONFIG_PATH: &str = "config/local.toml";
const CATALOG_SQL: &str = include_str!("../sql/postgres_catalog_compatibility.sql");
const PREPROCESSOR_SQL: &str = include_str!("../sql/exasol_sql_preprocessor.sql");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapMode {
    Interactive,
    Skip,
}

pub fn ensure_config_file(
    requested_path: Option<PathBuf>,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let path = requested_path.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    if path.exists() {
        return Ok(path);
    }

    if !io::stdin().is_terminal() {
        return Err(format!(
            "configuration file {} does not exist; run interactively once or pass --config",
            path.display()
        )
        .into());
    }

    println!("No configuration file found at {}.", path.display());
    if !confirm("Create it now?", true)? {
        return Err("configuration is required".into());
    }

    let content = prompt_config(&path)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    println!("Wrote configuration to {}.", path.display());
    print_systemd_guidance(&path);
    Ok(path)
}

pub fn run_interactive_bootstrap(
    config: &AppConfig,
    config_path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !io::stdin().is_terminal() {
        return Ok(());
    }

    println!();
    println!("Exasol PostgreSQL catalog bootstrap");
    println!(
        "The credentials entered here are used only for this installation check and are not saved."
    );
    println!("Normal SQL processing continues to use the credentials supplied by each client.");
    if !confirm("Check/install PG_CATALOG and INFORMATION_SCHEMA now?", true)? {
        print_systemd_guidance(config_path);
        return Ok(());
    }

    let username = prompt_with_default("Exasol setup user", "sys")?;
    let password = rpassword::prompt_password("Exasol setup password: ")?;
    let mut setup_config = config.exasol.clone();
    setup_config.pass_client_credentials = true;
    let mut session = ExasolSession::connect(&setup_config, &username, &password)?;

    let schemas = installed_schema_count(&mut session)?;
    if schemas < 2 {
        println!("Missing PostgreSQL compatibility schemas.");
        if confirm(
            "Create or replace PG_CATALOG and INFORMATION_SCHEMA compatibility objects?",
            true,
        )? {
            install_catalog(&mut session)?;
        } else {
            println!("Skipped catalog installation.");
        }
    } else if confirm(
        "PG_CATALOG and INFORMATION_SCHEMA are present. Refresh compatibility objects?",
        false,
    )? {
        install_catalog(&mut session)?;
    }

    if !config.translation.sql_preprocessor_script.trim().is_empty() {
        ensure_optional_preprocessor(&mut session, &config.translation.sql_preprocessor_script)?;
    }

    print_systemd_guidance(config_path);
    Ok(())
}

fn prompt_config(path: &Path) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    println!("Config path: {}", path.display());
    let listen_host = prompt_with_default("PostgreSQL listener host", "0.0.0.0")?;
    let listen_port = loop {
        let value = prompt_with_default("PostgreSQL listener port", "15432")?;
        if value.parse::<u16>().is_ok() {
            break value;
        }
        println!("PostgreSQL listener port must be a number from 0 to 65535.");
    };
    let log_level = prompt_with_default("Log level", "INFO")?;
    let dsn = prompt_required("Exasol DSN (host:port)")?;
    let encryption = confirm("Use encrypted Exasol WebSocket connection?", true)?;
    let certificate_fingerprint =
        prompt_optional("Exasol certificate SHA-256 fingerprint (blank to skip)")?;
    let validate_certificate = if certificate_fingerprint.is_empty() {
        confirm("Validate Exasol TLS certificate?", true)?
    } else {
        true
    };
    let schema = prompt_optional("Initial Exasol schema for client sessions (blank for none)")?;

    Ok(format!(
        r#"[server]
listen_host = "{}"
listen_port = {}
log_level = "{}"

[exasol]
dsn = "{}"
encryption = {}
certificate_fingerprint = "{}"
validate_certificate = {}
pass_client_credentials = true
schema = "{}"

[translation]
enabled = true
"#,
        toml_escape(&listen_host),
        listen_port.trim(),
        toml_escape(&log_level),
        toml_escape(&dsn),
        encryption,
        toml_escape(&certificate_fingerprint),
        validate_certificate,
        toml_escape(&schema),
    ))
}

fn installed_schema_count(session: &mut ExasolSession) -> Result<usize, ExasolError> {
    let result = session.execute(
        "SELECT COUNT(*) FROM SYS.EXA_SCHEMAS WHERE SCHEMA_NAME IN ('PG_CATALOG', 'INFORMATION_SCHEMA')",
    )?;
    first_count(result)
}

fn ensure_optional_preprocessor(
    session: &mut ExasolSession,
    configured_script: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let script = configured_script.trim().to_ascii_uppercase();
    if script != "PG_CATALOG.PG_SQL_PREPROCESSOR" {
        println!(
            "Configured preprocessor {configured_script} is not managed by bootstrap; skipping preprocessor install."
        );
        return Ok(());
    }

    let result = session.execute(
        "SELECT COUNT(*) FROM SYS.EXA_ALL_SCRIPTS WHERE SCRIPT_SCHEMA = 'PG_CATALOG' AND SCRIPT_NAME = 'PG_SQL_PREPROCESSOR'",
    )?;
    if first_count(result)? == 1 {
        return Ok(());
    }

    if confirm(
        "Optional PG_CATALOG.PG_SQL_PREPROCESSOR fallback is configured but missing. Install it now?",
        false,
    )? {
        execute_exasol_script(session, PREPROCESSOR_SQL)?;
        println!("Installed PG_CATALOG.PG_SQL_PREPROCESSOR.");
    }
    Ok(())
}

fn install_catalog(session: &mut ExasolSession) -> Result<(), ExasolError> {
    println!("Installing PostgreSQL compatibility objects...");
    execute_exasol_script(session, CATALOG_SQL)?;
    println!("Installed PG_CATALOG and INFORMATION_SCHEMA compatibility objects.");
    Ok(())
}

fn execute_exasol_script(session: &mut ExasolSession, sql_text: &str) -> Result<(), ExasolError> {
    let statements = split_exasol_sql(sql_text);
    for (idx, statement) in statements.iter().enumerate() {
        println!(
            "[{}/{}] {}",
            idx + 1,
            statements.len(),
            preview_statement(statement)
        );
        session.execute(statement)?;
    }
    Ok(())
}

fn split_exasol_sql(sql_text: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut buffer = Vec::new();
    let mut in_compound_statement = false;

    for raw_line in sql_text.lines() {
        let stripped = raw_line.trim();

        if stripped == "--/" {
            continue;
        }

        if buffer.is_empty() && (stripped.is_empty() || stripped.starts_with("--")) {
            continue;
        }

        buffer.push(raw_line);

        let upper = stripped.to_ascii_uppercase();
        if upper.starts_with("CREATE")
            && (format!(" {upper} ").contains(" FUNCTION ")
                || format!(" {upper} ").contains(" SCRIPT "))
        {
            in_compound_statement = true;
        }

        if in_compound_statement && stripped == "/" {
            let statement = buffer[..buffer.len() - 1].join("\n").trim().to_owned();
            if !statement.is_empty() {
                statements.push(statement);
            }
            buffer.clear();
            in_compound_statement = false;
        } else if !in_compound_statement && stripped.ends_with(';') {
            let statement = buffer
                .join("\n")
                .trim()
                .trim_end_matches(';')
                .trim()
                .to_owned();
            if !statement.is_empty() {
                statements.push(statement);
            }
            buffer.clear();
        }
    }

    let tail = buffer
        .join("\n")
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_owned();
    if !tail.is_empty() {
        statements.push(tail);
    }

    statements
}

fn preview_statement(sql: &str) -> String {
    let one_line = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= 80 {
        return one_line;
    }

    let mut preview = one_line.chars().take(77).collect::<String>();
    preview.push_str("...");
    preview
}

fn first_count(result: ExasolResult) -> Result<usize, ExasolError> {
    let ExasolResult::ResultSet { rows, .. } = result else {
        return Err(ExasolError::Execution(
            "schema check returned row count instead of result set".to_owned(),
        ));
    };
    rows.first()
        .and_then(|row| row.first())
        .and_then(|value| value.as_deref())
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| ExasolError::Execution("schema check returned no count".to_owned()))
}

fn prompt_required(label: &str) -> io::Result<String> {
    loop {
        let value = prompt_optional(label)?;
        if !value.trim().is_empty() {
            return Ok(value);
        }
        println!("{label} is required.");
    }
}

fn prompt_with_default(label: &str, default: &str) -> io::Result<String> {
    let value = prompt_optional(&format!("{label} [{default}]"))?;
    if value.trim().is_empty() {
        Ok(default.to_owned())
    } else {
        Ok(value)
    }
}

fn prompt_optional(label: &str) -> io::Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
}

fn confirm(label: &str, default: bool) -> io::Result<bool> {
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        let value = prompt_optional(&format!("{label} {suffix}"))?;
        if value.is_empty() {
            return Ok(default);
        }
        match value.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please answer yes or no."),
        }
    }
}

fn print_systemd_guidance(config_path: &Path) {
    let binary = std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "/opt/exa-postgres-interface/bin/exa-postgres-interface".to_owned());
    println!();
    println!("System service guidance:");
    println!("  1. Copy the binary to /opt/exa-postgres-interface/bin/exa-postgres-interface.");
    println!(
        "  2. Copy {} to /etc/exa-postgres-interface/config.toml.",
        config_path.display()
    );
    println!("  3. Install packaging/exa-postgres-interface.service into /etc/systemd/system/.");
    println!("  4. Use an ExecStart like:");
    println!("     {binary} --config /etc/exa-postgres-interface/config.toml --no-bootstrap");
    println!(
        "  5. Run: sudo systemctl daemon-reload && sudo systemctl enable --now exa-postgres-interface"
    );
    println!();
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{CATALOG_SQL, split_exasol_sql};

    #[test]
    fn splits_semicolon_and_slash_terminated_statements() {
        let sql = r#"
CREATE SCHEMA IF NOT EXISTS PG_CATALOG;

--/
CREATE OR REPLACE FUNCTION PG_CATALOG.TEST_FUNC()
RETURN DECIMAL(18,0)
IS
BEGIN
    RETURN 1;
END TEST_FUNC;
/

CREATE OR REPLACE VIEW PG_CATALOG.TEST_VIEW AS
SELECT 1 AS VALUE;
"#;

        let statements = split_exasol_sql(sql);

        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0], "CREATE SCHEMA IF NOT EXISTS PG_CATALOG");
        assert!(statements[1].contains("RETURN 1;"));
        assert!(!statements[1].contains("\n/"));
        assert_eq!(
            statements[2],
            "CREATE OR REPLACE VIEW PG_CATALOG.TEST_VIEW AS\nSELECT 1 AS VALUE"
        );
    }

    #[test]
    fn splits_empty_input_to_no_statements() {
        assert!(split_exasol_sql("\n\n  \n").is_empty());
    }

    #[test]
    fn catalog_sql_is_split_into_individual_statements() {
        let statements = split_exasol_sql(CATALOG_SQL);

        assert!(statements.len() > 100);
        assert_eq!(statements[0], "CREATE SCHEMA IF NOT EXISTS PG_CATALOG");
        assert_eq!(
            statements[1],
            "CREATE SCHEMA IF NOT EXISTS INFORMATION_SCHEMA"
        );
        assert!(statements.iter().any(|statement| {
            statement.starts_with("CREATE OR REPLACE FUNCTION PG_CATALOG.OBJ_DESCRIPTION")
        }));
        assert!(statements.iter().all(|statement| statement.trim() != "/"));
    }
}
