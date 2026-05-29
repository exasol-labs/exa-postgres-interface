use std::fs;
use std::path::Path;

use serde::Deserialize;

pub const DEFAULT_TRANSPORT: &str = "arrow";

fn default_transport() -> String {
    DEFAULT_TRANSPORT.to_owned()
}

/// The data-transport protocol used to connect to Exasol.
#[derive(Debug, Clone, PartialEq)]
pub enum Transport {
    WebSocket,
    Arrow,
}

impl Transport {
    /// Parse transport from the config value, returning a clear error for unknown variants.
    pub fn from_config(
        config: &ExasolConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match config.transport.as_str() {
            "websocket" => Ok(Transport::WebSocket),
            "arrow" => Ok(Transport::Arrow),
            other => Err(format!(
                "unknown transport '{}': accepted values are 'websocket' and 'arrow'",
                other
            )
            .into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    pub exasol: ExasolConfig,
    #[serde(default)]
    pub translation: TranslationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_listen_host")]
    pub listen_host: String,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub tls_cert_path: String,
    #[serde(default)]
    pub tls_key_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExasolConfig {
    pub dsn: String,
    #[serde(default = "default_true")]
    pub encryption: bool,
    #[serde(default)]
    pub certificate_fingerprint: String,
    #[serde(default = "default_true")]
    pub validate_certificate: bool,
    #[serde(default = "default_true")]
    pub pass_client_credentials: bool,
    #[serde(default)]
    pub schema: String,
    #[serde(default = "default_transport")]
    pub transport: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranslationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sql_preprocessor_script: String,
    #[serde(default)]
    pub session_init_sql: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_host: default_listen_host(),
            listen_port: default_listen_port(),
            log_level: default_log_level(),
            tls_cert_path: String::new(),
            tls_key_path: String::new(),
        }
    }
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sql_preprocessor_script: String::new(),
            session_init_sql: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn from_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        if config.exasol.dsn.trim().is_empty() {
            return Err("exasol.dsn is required".into());
        }
        if config.server.tls_cert_path.trim().is_empty()
            != config.server.tls_key_path.trim().is_empty()
        {
            return Err("server.tls_cert_path and server.tls_key_path must be set together".into());
        }
        Transport::from_config(&config.exasol)?;
        Ok(config)
    }

    pub fn log_filter(&self) -> String {
        format!(
            "exa_postgres_interface={},pgwire=info",
            self.server.log_level
        )
    }
}

fn default_listen_host() -> String {
    "127.0.0.1".to_owned()
}

fn default_listen_port() -> u16 {
    15432
}

fn default_log_level() -> String {
    "info".to_owned()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_config() {
        let raw = r#"
            [server]
            listen_host = "0.0.0.0"
            listen_port = 15432
            tls_cert_path = "/etc/exa-postgres-interface/server.crt"
            tls_key_path = "/etc/exa-postgres-interface/server.key"

            [exasol]
            dsn = "127.0.0.1:8563"
            validate_certificate = false

            [translation]
            enabled = true
        "#;

        let config: AppConfig = toml::from_str(raw).unwrap();

        assert_eq!(config.server.listen_host, "0.0.0.0");
        assert_eq!(
            config.server.tls_cert_path,
            "/etc/exa-postgres-interface/server.crt"
        );
        assert_eq!(config.exasol.dsn, "127.0.0.1:8563");
        assert!(!config.exasol.validate_certificate);
        assert!(config.translation.enabled);
        assert!(config.translation.sql_preprocessor_script.is_empty());
        assert!(config.translation.session_init_sql.is_empty());
    }

    #[test]
    fn loads_optional_preprocessor_fallback_config() {
        let raw = r#"
            [exasol]
            dsn = "127.0.0.1:8563"

            [translation]
            enabled = true
            sql_preprocessor_script = "PG_CATALOG.PG_SQL_PREPROCESSOR"
            session_init_sql = ["ALTER SESSION SET SQL_PREPROCESSOR_SCRIPT = {script}"]
        "#;

        let config: AppConfig = toml::from_str(raw).unwrap();

        assert_eq!(
            config.translation.sql_preprocessor_script,
            "PG_CATALOG.PG_SQL_PREPROCESSOR"
        );
        assert_eq!(config.translation.session_init_sql.len(), 1);
    }

    #[test]
    fn transport_defaults_to_arrow() {
        let raw = "[exasol]\ndsn = \"127.0.0.1:8563\"\n";
        let config: AppConfig = toml::from_str(raw).unwrap();
        assert_eq!(config.exasol.transport, DEFAULT_TRANSPORT);
        assert_eq!(
            Transport::from_config(&config.exasol).unwrap(),
            Transport::Arrow
        );
    }

    #[test]
    fn explicit_websocket_transport_parses() {
        let raw = "[exasol]\ndsn = \"127.0.0.1:8563\"\ntransport = \"websocket\"\n";
        let config: AppConfig = toml::from_str(raw).unwrap();
        assert_eq!(
            Transport::from_config(&config.exasol).unwrap(),
            Transport::WebSocket
        );
    }

    #[test]
    fn explicit_arrow_transport_parses() {
        let raw = "[exasol]\ndsn = \"127.0.0.1:8563\"\ntransport = \"arrow\"\n";
        let config: AppConfig = toml::from_str(raw).unwrap();
        assert_eq!(
            Transport::from_config(&config.exasol).unwrap(),
            Transport::Arrow
        );
    }

    #[test]
    fn unknown_transport_value_fails_config_load() {
        let raw = "[exasol]\ndsn = \"127.0.0.1:8563\"\ntransport = \"tcp\"\n";
        let config: AppConfig = toml::from_str(raw).unwrap();
        let err = Transport::from_config(&config.exasol).unwrap_err();
        assert!(err.to_string().contains("unknown transport"));
        assert!(err.to_string().contains("websocket"));
        assert!(err.to_string().contains("arrow"));
    }

    #[test]
    fn from_file_rejects_unknown_transport() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            "[exasol]\ndsn = \"127.0.0.1:8563\"\ntransport = \"tcp\""
        )
        .unwrap();
        let err = AppConfig::from_file(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("unknown transport"));
    }
}
