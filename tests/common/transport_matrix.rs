//! Transport-parameterisation harness for integration tests.
//!
//! `all_transport_configs` returns one `TransportTestConfig` per transport
//! variant so that every integration test can exercise both the WebSocket JSON
//! path and the Arrow path without duplicating test logic.

use exa_postgres_interface::config::{AppConfig, ExasolConfig, ServerConfig, TranslationConfig};

pub struct TransportTestConfig {
    pub transport: &'static str,
    pub exasol_config: ExasolConfig,
}

impl TransportTestConfig {
    pub fn app_config(&self, listen_port: u16) -> AppConfig {
        AppConfig {
            server: ServerConfig {
                listen_host: "127.0.0.1".to_owned(),
                listen_port,
                log_level: "info".to_owned(),
                tls_cert_path: String::new(),
                tls_key_path: String::new(),
            },
            exasol: self.exasol_config.clone(),
            translation: TranslationConfig {
                enabled: false,
                sql_preprocessor_script: String::new(),
                session_init_sql: Vec::new(),
            },
        }
    }
}

pub fn all_transport_configs() -> Vec<TransportTestConfig> {
    vec![
        TransportTestConfig {
            transport: "websocket",
            exasol_config: ExasolConfig {
                transport: "websocket".to_owned(),
                ..super::live_exasol_config()
            },
        },
        TransportTestConfig {
            transport: "arrow",
            exasol_config: ExasolConfig {
                transport: "arrow".to_owned(),
                ..super::live_exasol_config()
            },
        },
    ]
}
