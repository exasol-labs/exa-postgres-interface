use arrow::array::RecordBatch;
use async_trait::async_trait;
use exarrow_rs::connection::ConnectionParams;
use exarrow_rs::error::ConnectionError;
use thiserror::Error;

use crate::config::ExasolConfig;

pub(crate) mod arrow_transport;
pub(crate) mod websocket_transport;

use arrow_transport::ArrowTransport;
use websocket_transport::WebSocketTransport;

#[derive(Debug, Error)]
pub enum ExasolError {
    #[error("invalid Exasol DSN: {0}")]
    InvalidDsn(String),
    #[error("Exasol connection failed: {0}")]
    Connection(String),
    #[error("Exasol authentication failed: {0}")]
    Authentication(String),
    #[error("Exasol request failed: {0}")]
    Request(String),
    #[error("Exasol execution failed: {0}")]
    Execution(String),
}

pub(crate) fn map_connection_error(err: ConnectionError) -> ExasolError {
    match err {
        ConnectionError::AuthenticationFailed(msg) => ExasolError::Authentication(msg),
        other => ExasolError::Connection(other.to_string()),
    }
}

/// Column metadata returned by the WebSocket transport.
#[derive(Debug, Clone)]
pub struct ExasolColumn {
    pub name: String,
    pub data_type: serde_json::Value,
}

/// The outcome of an Exasol statement execution.
#[derive(Debug)]
pub enum ExasolOutcome {
    /// Arrow transport result: record batches.
    ArrowRows(Vec<RecordBatch>),
    /// WebSocket transport result: typed string rows.
    TypedRows {
        columns: Vec<ExasolColumn>,
        rows: Vec<Vec<Option<String>>>,
    },
    /// A DML statement affected this many rows.
    RowCount(i64),
}

/// Async, transport-agnostic dispatch contract for Exasol statement execution.
///
/// One implementation wraps `exarrow_rs::Connection` (Arrow transport); the
/// other wraps a `tokio_tungstenite::WebSocketStream` (WebSocket JSON
/// transport). `ExasolSession` holds the trait object so every call site is
/// transport-agnostic.
#[async_trait]
pub(crate) trait ExasolTransport: Send {
    async fn execute(&mut self, sql: &str) -> Result<ExasolOutcome, ExasolError>;
    async fn execute_update(&mut self, sql: &str) -> Result<(), ExasolError>;
    async fn close(self: Box<Self>) -> Result<(), ExasolError>;
}

/// A thin facade over a chosen `ExasolTransport` implementation.
pub struct ExasolSession {
    inner: Box<dyn ExasolTransport>,
}

impl ExasolSession {
    pub async fn connect(
        config: &ExasolConfig,
        username: &str,
        password: &str,
    ) -> Result<Self, ExasolError> {
        let endpoint = EndpointConnection::parse(&config.dsn, config)?;
        let transport = crate::config::Transport::from_config(config)
            .map_err(|err| ExasolError::Connection(err.to_string()))?;
        let inner: Box<dyn ExasolTransport> = match transport {
            crate::config::Transport::Arrow => {
                tracing::info!("selected transport: arrow");
                Box::new(ArrowTransport::connect(config, &endpoint, username, password).await?)
            }
            crate::config::Transport::WebSocket => {
                tracing::info!("selected transport: websocket");
                Box::new(WebSocketTransport::connect(config, &endpoint, username, password).await?)
            }
        };
        Ok(Self { inner })
    }

    /// Run each `session_init_sql` template, substituting `{script}` with `script`.
    pub async fn initialize(
        &mut self,
        session_init_sql: &[String],
        script: &str,
    ) -> Result<(), ExasolError> {
        for template in session_init_sql {
            let sql = template.replace("{script}", script);
            tracing::info!("running configured Exasol session initialization SQL");
            self.inner.execute_update(&sql).await?;
        }
        Ok(())
    }

    /// Execute `sql` and return the result as an `ExasolOutcome`.
    pub async fn execute(&mut self, sql: &str) -> Result<ExasolOutcome, ExasolError> {
        self.inner.execute(sql).await
    }

    /// Execute `sql` expecting a DML / `rowCount` result with no rows returned.
    pub async fn execute_update(&mut self, sql: &str) -> Result<(), ExasolError> {
        self.inner.execute_update(sql).await
    }

    /// Gracefully close the session, consuming `self`.
    pub async fn close(self) -> Result<(), ExasolError> {
        self.inner.close().await
    }
}

/// Parsed connection endpoint derived from an Exasol DSN string.
///
/// Transport-neutral: both `ArrowTransport` and `WebSocketTransport` consume
/// the same shape so DSN-fingerprint precedence and `NOCERTCHECK` behave
/// identically on either path.
#[derive(Debug)]
pub struct EndpointConnection {
    pub host: String,
    pub port: u16,
    /// `None` means validate normally; `Some("NOCERTCHECK")` disables validation;
    /// any other value is treated as a SHA-256 fingerprint to pin against.
    pub fingerprint: Option<String>,
}

impl EndpointConnection {
    pub fn parse(dsn: &str, config: &ExasolConfig) -> Result<Self, ExasolError> {
        let first = dsn
            .split(',')
            .next()
            .ok_or_else(|| ExasolError::InvalidDsn(dsn.to_owned()))?
            .trim();
        let (host_and_fingerprint, port) = first
            .rsplit_once(':')
            .ok_or_else(|| ExasolError::InvalidDsn(dsn.to_owned()))?;
        let (host, dsn_fingerprint) = match host_and_fingerprint.split_once('/') {
            Some((host, fingerprint)) => (host.to_owned(), Some(fingerprint.to_owned())),
            None => (host_and_fingerprint.to_owned(), None),
        };
        let fingerprint = if !config.certificate_fingerprint.trim().is_empty() {
            Some(config.certificate_fingerprint.trim().to_ascii_uppercase())
        } else if let Some(fingerprint) = dsn_fingerprint {
            Some(fingerprint.to_ascii_uppercase())
        } else if !config.validate_certificate {
            Some("NOCERTCHECK".to_owned())
        } else {
            None
        };
        Ok(Self {
            host,
            port: port
                .parse()
                .map_err(|_| ExasolError::InvalidDsn(dsn.to_owned()))?,
            fingerprint,
        })
    }

    /// Build `ConnectionParams` from this endpoint, config, and credentials.
    ///
    /// The `NOCERTCHECK` sentinel disables certificate validation on the driver.
    /// Any other fingerprint value enables pinning against the specified SHA-256 hex digest.
    pub fn to_connection_params(
        &self,
        config: &ExasolConfig,
        username: &str,
        password: &str,
    ) -> Result<ConnectionParams, ExasolError> {
        use exarrow_rs::connection::ConnectionBuilder;

        let (validate_cert, pin_fingerprint) = match &self.fingerprint {
            Some(fp) if fp == "NOCERTCHECK" => (false, None),
            Some(fp) => (config.validate_certificate, Some(fp.as_str())),
            None => (config.validate_certificate, None),
        };

        let mut builder = ConnectionBuilder::new()
            .host(&self.host)
            .port(self.port)
            .username(username)
            .password(password)
            .use_tls(config.encryption)
            .validate_server_certificate(validate_cert);

        if let Some(fp) = pin_fingerprint {
            builder = builder.certificate_fingerprint(fp);
        }

        if !config.schema.is_empty() {
            builder = builder.schema(&config.schema);
        }

        builder
            .build()
            .map_err(|err| ExasolError::Connection(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_nocertcheck_policy_from_config() {
        let config = ExasolConfig {
            dsn: "127.0.0.1:8563".to_owned(),
            encryption: true,
            certificate_fingerprint: String::new(),
            validate_certificate: false,
            pass_client_credentials: true,
            schema: String::new(),
            transport: String::new(),
        };

        let endpoint = EndpointConnection::parse(&config.dsn, &config).unwrap();

        assert_eq!(endpoint.fingerprint.as_deref(), Some("NOCERTCHECK"));
    }

    #[test]
    fn preserves_dsn_fingerprint() {
        let config = ExasolConfig {
            dsn: "127.0.0.1/ABC:8563".to_owned(),
            encryption: true,
            certificate_fingerprint: String::new(),
            validate_certificate: true,
            pass_client_credentials: true,
            schema: String::new(),
            transport: String::new(),
        };

        let endpoint = EndpointConnection::parse(&config.dsn, &config).unwrap();

        assert_eq!(endpoint.fingerprint.as_deref(), Some("ABC"));
    }

    #[test]
    fn nocertcheck_maps_to_validate_false_on_connection_params() {
        let config = ExasolConfig {
            dsn: "127.0.0.1:8563".to_owned(),
            encryption: true,
            certificate_fingerprint: String::new(),
            validate_certificate: false,
            pass_client_credentials: true,
            schema: String::new(),
            transport: String::new(),
        };

        let endpoint = EndpointConnection::parse(&config.dsn, &config).unwrap();
        let params = endpoint
            .to_connection_params(&config, "user", "pass")
            .unwrap();

        assert!(!params.validate_server_certificate);
        assert!(params.certificate_fingerprint.is_none());
    }

    #[test]
    fn dsn_fingerprint_propagates_to_connection_params() {
        let config = ExasolConfig {
            dsn: "127.0.0.1/AABBCC:8563".to_owned(),
            encryption: true,
            certificate_fingerprint: String::new(),
            validate_certificate: true,
            pass_client_credentials: true,
            schema: String::new(),
            transport: String::new(),
        };

        let endpoint = EndpointConnection::parse(&config.dsn, &config).unwrap();
        let params = endpoint
            .to_connection_params(&config, "user", "pass")
            .unwrap();

        assert_eq!(params.certificate_fingerprint.as_deref(), Some("AABBCC"));
    }

    #[test]
    fn config_fingerprint_overrides_dsn_fingerprint() {
        let config = ExasolConfig {
            dsn: "127.0.0.1/DSN_FP:8563".to_owned(),
            encryption: true,
            certificate_fingerprint: "CONFIG_FP".to_owned(),
            validate_certificate: true,
            pass_client_credentials: true,
            schema: String::new(),
            transport: String::new(),
        };

        let endpoint = EndpointConnection::parse(&config.dsn, &config).unwrap();

        assert_eq!(endpoint.fingerprint.as_deref(), Some("CONFIG_FP"));
    }
}
