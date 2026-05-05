use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use native_tls::{Certificate, TlsConnector, TlsStream};
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs1v15::Pkcs1v15Encrypt;
use rsa::rand_core::OsRng;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tungstenite::{Message, WebSocket};

use crate::config::ExasolConfig;

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

#[derive(Debug, Clone)]
pub struct ExasolColumn {
    pub name: String,
    pub data_type: Value,
}

#[derive(Debug, Clone)]
pub enum ExasolResult {
    ResultSet {
        columns: Vec<ExasolColumn>,
        rows: Vec<Vec<Option<String>>>,
    },
    RowCount(usize),
}

enum ExaStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl Read for ExaStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ExaStream::Plain(stream) => stream.read(buf),
            ExaStream::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for ExaStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ExaStream::Plain(stream) => stream.write(buf),
            ExaStream::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ExaStream::Plain(stream) => stream.flush(),
            ExaStream::Tls(stream) => stream.flush(),
        }
    }
}

pub struct ExasolSession {
    ws: WebSocket<ExaStream>,
}

impl ExasolSession {
    pub fn connect(
        config: &ExasolConfig,
        username: &str,
        password: &str,
    ) -> Result<Self, ExasolError> {
        if !config.pass_client_credentials {
            return Err(ExasolError::Authentication(
                "only client credential passthrough is implemented".to_owned(),
            ));
        }

        let endpoint = Endpoint::parse(&config.dsn, config)?;
        let stream = connect_stream(&endpoint, config)?;
        let scheme = if config.encryption { "wss" } else { "ws" };
        let url = format!("{scheme}://{}:{}", endpoint.host, endpoint.port);
        let (ws, _) = tungstenite::client::client(url.as_str(), stream)
            .map_err(|err| ExasolError::Connection(err.to_string()))?;
        let mut session = Self { ws };
        session.login(config, username, password)?;
        Ok(session)
    }

    pub fn initialize(
        &mut self,
        session_init_sql: &[String],
        script: &str,
    ) -> Result<(), ExasolError> {
        for template in session_init_sql {
            let sql = template.replace("{script}", script);
            tracing::info!("running configured Exasol session initialization SQL");
            self.execute(&sql)?;
        }
        Ok(())
    }

    pub fn execute(&mut self, sql: &str) -> Result<ExasolResult, ExasolError> {
        let ret = self.request(json!({
            "command": "execute",
            "sqlText": sql,
        }))?;

        let result = ret
            .pointer("/responseData/results/0")
            .ok_or_else(|| ExasolError::Execution("missing execute result".to_owned()))?;

        parse_result(result, self)
    }

    fn login(
        &mut self,
        config: &ExasolConfig,
        username: &str,
        password: &str,
    ) -> Result<(), ExasolError> {
        let public_key_ret = self.request(json!({
            "command": "login",
            "protocolVersion": 3,
        }))?;
        let public_key_pem = public_key_ret
            .pointer("/responseData/publicKeyPem")
            .and_then(Value::as_str)
            .ok_or_else(|| ExasolError::Authentication("missing Exasol public key".to_owned()))?;
        let encrypted_password = encrypt_password(public_key_pem, password)?;

        let mut attributes = json!({
            "currentSchema": config.schema,
            "autocommit": true,
            "queryTimeout": 0,
        });
        if config.schema.is_empty() {
            attributes["currentSchema"] = Value::String(String::new());
        }

        self.request(json!({
            "username": username,
            "password": encrypted_password,
            "driverName": "exa-postgres-interface",
            "clientName": "exa-postgres-interface",
            "clientVersion": env!("CARGO_PKG_VERSION"),
            "clientOs": std::env::consts::OS,
            "clientOsUsername": std::env::var("USER").unwrap_or_else(|_| "unknown".to_owned()),
            "clientRuntime": "Rust",
            "useCompression": false,
            "attributes": attributes,
        }))
        .map_err(|err| ExasolError::Authentication(err.to_string()))?;

        Ok(())
    }

    fn request(&mut self, request: Value) -> Result<Value, ExasolError> {
        let payload =
            serde_json::to_string(&request).map_err(|err| ExasolError::Request(err.to_string()))?;
        self.ws
            .send(Message::Text(payload))
            .map_err(|err| ExasolError::Request(err.to_string()))?;

        let text = self.read_json_response()?;
        let response: Value =
            serde_json::from_str(&text).map_err(|err| ExasolError::Request(err.to_string()))?;

        if response.get("status").and_then(Value::as_str) == Some("ok") {
            Ok(response)
        } else {
            let code = response
                .pointer("/exception/sqlCode")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let text = response
                .pointer("/exception/text")
                .and_then(Value::as_str)
                .unwrap_or("unknown Exasol error");
            Err(ExasolError::Execution(format!("{text} (SQL code: {code})")))
        }
    }

    fn read_json_response(&mut self) -> Result<String, ExasolError> {
        loop {
            let message = self
                .ws
                .read()
                .map_err(|err| ExasolError::Request(err.to_string()))?;
            match message {
                Message::Ping(payload) => {
                    self.ws
                        .send(Message::Pong(payload))
                        .map_err(|err| ExasolError::Request(err.to_string()))?;
                }
                other => {
                    if let Some(text) = response_text_from_message(other)? {
                        return Ok(text);
                    }
                }
            }
        }
    }
}

fn response_text_from_message(message: Message) -> Result<Option<String>, ExasolError> {
    match message {
        Message::Text(text) => Ok(Some(text)),
        Message::Binary(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|err| ExasolError::Request(format!("invalid UTF-8 response: {err}"))),
        Message::Pong(payload) => {
            tracing::debug!(
                payload = %String::from_utf8_lossy(&payload),
                "ignoring Exasol websocket pong/progress frame"
            );
            Ok(None)
        }
        Message::Frame(_) => Ok(None),
        Message::Ping(_) => Ok(None),
        Message::Close(close) => Err(ExasolError::Request(format!(
            "Exasol closed websocket while waiting for response: {close:?}"
        ))),
    }
}

impl Drop for ExasolSession {
    fn drop(&mut self) {
        let _ = self.request(json!({ "command": "disconnect" }));
        let _ = self.ws.close(None);
    }
}

#[derive(Debug)]
struct Endpoint {
    host: String,
    port: u16,
    fingerprint: Option<String>,
}

impl Endpoint {
    fn parse(dsn: &str, config: &ExasolConfig) -> Result<Self, ExasolError> {
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
}

fn connect_stream(endpoint: &Endpoint, config: &ExasolConfig) -> Result<ExaStream, ExasolError> {
    let tcp = TcpStream::connect((endpoint.host.as_str(), endpoint.port))
        .map_err(|err| ExasolError::Connection(err.to_string()))?;
    tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();

    if !config.encryption {
        return Ok(ExaStream::Plain(tcp));
    }

    let mut builder = TlsConnector::builder();
    if endpoint.fingerprint.is_some() {
        builder.danger_accept_invalid_certs(true);
    }
    let connector = builder
        .build()
        .map_err(|err| ExasolError::Connection(err.to_string()))?;
    let stream = connector
        .connect(endpoint.host.as_str(), tcp)
        .map_err(|err| ExasolError::Connection(err.to_string()))?;

    if let Some(fingerprint) = &endpoint.fingerprint {
        if fingerprint != "NOCERTCHECK" {
            verify_fingerprint(&stream, fingerprint)?;
        }
    }

    Ok(ExaStream::Tls(stream))
}

fn verify_fingerprint(stream: &TlsStream<TcpStream>, expected: &str) -> Result<(), ExasolError> {
    let cert = stream
        .peer_certificate()
        .map_err(|err| ExasolError::Connection(err.to_string()))?
        .ok_or_else(|| {
            ExasolError::Connection("server did not present a certificate".to_owned())
        })?;
    let actual = certificate_sha256_hex(&cert)?;
    if actual != expected.to_ascii_uppercase() {
        return Err(ExasolError::Connection(format!(
            "certificate fingerprint mismatch: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn certificate_sha256_hex(cert: &Certificate) -> Result<String, ExasolError> {
    let der = cert
        .to_der()
        .map_err(|err| ExasolError::Connection(err.to_string()))?;
    Ok(format!("{:X}", Sha256::digest(&der)))
}

fn encrypt_password(public_key_pem: &str, password: &str) -> Result<String, ExasolError> {
    let key = RsaPublicKey::from_pkcs1_pem(public_key_pem)
        .map_err(|err| ExasolError::Authentication(err.to_string()))?;
    let mut rng = OsRng;
    let encrypted = key
        .encrypt(&mut rng, Pkcs1v15Encrypt, password.as_bytes())
        .map_err(|err| ExasolError::Authentication(err.to_string()))?;
    Ok(BASE64.encode(encrypted))
}

fn parse_result(result: &Value, session: &mut ExasolSession) -> Result<ExasolResult, ExasolError> {
    match result
        .get("resultType")
        .and_then(Value::as_str)
        .ok_or_else(|| ExasolError::Execution("missing resultType".to_owned()))?
    {
        "rowCount" => Ok(ExasolResult::RowCount(
            result.get("rowCount").and_then(Value::as_u64).unwrap_or(0) as usize,
        )),
        "resultSet" => parse_result_set(result, session),
        other => Err(ExasolError::Execution(format!(
            "unsupported resultType: {other}"
        ))),
    }
}

fn parse_result_set(
    result: &Value,
    session: &mut ExasolSession,
) -> Result<ExasolResult, ExasolError> {
    let result_set = result
        .get("resultSet")
        .ok_or_else(|| ExasolError::Execution("missing resultSet".to_owned()))?;
    let columns = parse_columns(result_set)?;
    let total_rows = result_set
        .get("numRows")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let mut rows = transpose_data(result_set.get("data"));
    let mut fetched = result_set
        .get("numRowsInMessage")
        .and_then(Value::as_u64)
        .unwrap_or(rows.len() as u64) as usize;

    if let Some(handle) = result_set.get("resultSetHandle").and_then(Value::as_u64) {
        while fetched < total_rows {
            let ret = session.request(json!({
                "command": "fetch",
                "resultSetHandle": handle,
                "startPosition": fetched,
                "numBytes": 5_242_880,
            }))?;
            let chunk = ret
                .get("responseData")
                .ok_or_else(|| ExasolError::Execution("missing fetch responseData".to_owned()))?;
            let mut chunk_rows = transpose_data(chunk.get("data"));
            fetched += chunk
                .get("numRows")
                .and_then(Value::as_u64)
                .unwrap_or(chunk_rows.len() as u64) as usize;
            rows.append(&mut chunk_rows);
        }
    }

    Ok(ExasolResult::ResultSet { columns, rows })
}

fn parse_columns(result_set: &Value) -> Result<Vec<ExasolColumn>, ExasolError> {
    let columns = result_set
        .get("columns")
        .and_then(Value::as_array)
        .ok_or_else(|| ExasolError::Execution("missing resultSet columns".to_owned()))?;
    Ok(columns
        .iter()
        .map(|column| ExasolColumn {
            name: column
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("column")
                .to_owned(),
            data_type: column.get("dataType").cloned().unwrap_or(Value::Null),
        })
        .collect())
}

fn transpose_data(data: Option<&Value>) -> Vec<Vec<Option<String>>> {
    let Some(columns) = data.and_then(Value::as_array) else {
        return Vec::new();
    };
    let row_count = columns
        .iter()
        .filter_map(Value::as_array)
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    let mut rows = Vec::with_capacity(row_count);
    for row_idx in 0..row_count {
        let mut row = Vec::with_capacity(columns.len());
        for column in columns {
            let value = column.as_array().and_then(|values| values.get(row_idx));
            row.push(value_to_text(value));
        }
        rows.push(row);
    }
    rows
}

fn value_to_text(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Bool(b)) => Some(if *b { "t" } else { "f" }.to_owned()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(other) => Some(other.to_string()),
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
        };

        let endpoint = Endpoint::parse(&config.dsn, &config).unwrap();

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
        };

        let endpoint = Endpoint::parse(&config.dsn, &config).unwrap();

        assert_eq!(endpoint.fingerprint.as_deref(), Some("ABC"));
    }

    #[test]
    fn skips_exasol_pong_progress_frame() {
        assert!(
            response_text_from_message(Message::Pong(b"EXECUTING".to_vec()))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn accepts_text_response_frame() {
        assert_eq!(
            response_text_from_message(Message::Text(r#"{"status":"ok"}"#.to_owned())).unwrap(),
            Some(r#"{"status":"ok"}"#.to_owned())
        );
    }
}
