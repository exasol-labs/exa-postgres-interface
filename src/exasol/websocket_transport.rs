use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures::{SinkExt, StreamExt};
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs1v15::Pkcs1v15Encrypt;
use rsa::rand_core::OsRng;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream, client_async_tls_with_config};

use crate::config::ExasolConfig;
use crate::exasol::{
    EndpointConnection, ExasolColumn, ExasolError, ExasolOutcome, ExasolTransport,
};

/// Async port of the pre-migration hand-rolled Exasol WebSocket transport,
/// rebuilt on `tokio-tungstenite` and `tokio-rustls` so the same call site
/// satisfies the `ExasolTransport` async contract as the Arrow path.
pub(crate) struct WebSocketTransport {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WebSocketTransport {
    pub(crate) async fn connect(
        config: &ExasolConfig,
        endpoint: &EndpointConnection,
        username: &str,
        password: &str,
    ) -> Result<Self, ExasolError> {
        if !config.pass_client_credentials {
            return Err(ExasolError::Authentication(
                "only client credential passthrough is implemented".to_owned(),
            ));
        }

        let tcp = TcpStream::connect((endpoint.host.as_str(), endpoint.port))
            .await
            .map_err(|err| ExasolError::Connection(err.to_string()))?;

        let scheme = if config.encryption { "wss" } else { "ws" };
        let url = format!("{scheme}://{}:{}/", endpoint.host, endpoint.port);

        let connector = build_connector(config, endpoint)?;

        let (ws, _response) = client_async_tls_with_config(url.as_str(), tcp, None, connector)
            .await
            .map_err(|err| ExasolError::Connection(err.to_string()))?;

        let mut session = Self { ws };
        session.login(config, username, password).await?;
        Ok(session)
    }

    async fn login(
        &mut self,
        config: &ExasolConfig,
        username: &str,
        password: &str,
    ) -> Result<(), ExasolError> {
        let public_key_ret = self
            .request(json!({
                "command": "login",
                "protocolVersion": 3,
            }))
            .await?;
        let public_key_pem = public_key_ret
            .pointer("/responseData/publicKeyPem")
            .and_then(Value::as_str)
            .ok_or_else(|| ExasolError::Authentication("missing Exasol public key".to_owned()))?;
        let encrypted_password = encrypt_password(public_key_pem, password)?;

        let attributes = json!({
            "currentSchema": config.schema,
            "autocommit": true,
            "queryTimeout": 0,
        });

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
        .await
        .map_err(|err| ExasolError::Authentication(err.to_string()))?;

        Ok(())
    }

    async fn request(&mut self, request: Value) -> Result<Value, ExasolError> {
        let payload =
            serde_json::to_string(&request).map_err(|err| ExasolError::Request(err.to_string()))?;
        self.ws
            .send(Message::Text(payload))
            .await
            .map_err(|err| ExasolError::Request(err.to_string()))?;

        let text = self.read_json_response().await?;
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

    async fn read_json_response(&mut self) -> Result<String, ExasolError> {
        loop {
            let message = self
                .ws
                .next()
                .await
                .ok_or_else(|| {
                    ExasolError::Request(
                        "Exasol closed websocket while waiting for response".to_owned(),
                    )
                })?
                .map_err(|err| ExasolError::Request(err.to_string()))?;
            match message {
                Message::Ping(payload) => {
                    self.ws
                        .send(Message::Pong(payload))
                        .await
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

#[async_trait]
impl ExasolTransport for WebSocketTransport {
    async fn execute(&mut self, sql: &str) -> Result<ExasolOutcome, ExasolError> {
        let ret = self
            .request(json!({
                "command": "execute",
                "sqlText": sql,
            }))
            .await?;

        let result = ret
            .pointer("/responseData/results/0")
            .ok_or_else(|| ExasolError::Execution("missing execute result".to_owned()))?
            .clone();

        match result
            .get("resultType")
            .and_then(Value::as_str)
            .ok_or_else(|| ExasolError::Execution("missing resultType".to_owned()))?
        {
            "rowCount" => Ok(ExasolOutcome::RowCount(
                result.get("rowCount").and_then(Value::as_i64).unwrap_or(0),
            )),
            "resultSet" => parse_result_set(self, &result).await,
            other => Err(ExasolError::Execution(format!(
                "unsupported resultType: {other}"
            ))),
        }
    }

    async fn execute_update(&mut self, sql: &str) -> Result<(), ExasolError> {
        match self.execute(sql).await? {
            ExasolOutcome::RowCount(_) => Ok(()),
            ExasolOutcome::ArrowRows(_) | ExasolOutcome::TypedRows { .. } => {
                Err(ExasolError::Execution(
                    "execute_update received a result-set response; expected row count only"
                        .to_string(),
                ))
            }
        }
    }

    async fn close(mut self: Box<Self>) -> Result<(), ExasolError> {
        let _ = self
            .ws
            .send(Message::Text(
                serde_json::to_string(&json!({ "command": "disconnect" }))
                    .map_err(|err| ExasolError::Request(err.to_string()))?,
            ))
            .await;
        let _ = self.ws.close(None).await;
        Ok(())
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

async fn parse_result_set(
    session: &mut WebSocketTransport,
    result: &Value,
) -> Result<ExasolOutcome, ExasolError> {
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
            let ret = session
                .request(json!({
                    "command": "fetch",
                    "resultSetHandle": handle,
                    "startPosition": fetched,
                    "numBytes": 5_242_880u64,
                }))
                .await?;
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

    Ok(ExasolOutcome::TypedRows { columns, rows })
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

fn encrypt_password(public_key_pem: &str, password: &str) -> Result<String, ExasolError> {
    let key = RsaPublicKey::from_pkcs1_pem(public_key_pem)
        .map_err(|err| ExasolError::Authentication(err.to_string()))?;
    let mut rng = OsRng;
    let encrypted = key
        .encrypt(&mut rng, Pkcs1v15Encrypt, password.as_bytes())
        .map_err(|err| ExasolError::Authentication(err.to_string()))?;
    Ok(BASE64.encode(encrypted))
}

fn certificate_sha256_hex(cert_der: &[u8]) -> String {
    format!("{:X}", Sha256::digest(cert_der))
}

fn verify_fingerprint(expected: &str, cert_der: &[u8]) -> Result<(), ExasolError> {
    let actual = certificate_sha256_hex(cert_der);
    if actual != expected.to_ascii_uppercase() {
        return Err(ExasolError::Connection(format!(
            "certificate fingerprint mismatch: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn build_connector(
    config: &ExasolConfig,
    endpoint: &EndpointConnection,
) -> Result<Option<Connector>, ExasolError> {
    if !config.encryption {
        return Ok(Some(Connector::Plain));
    }

    let client_config = match &endpoint.fingerprint {
        Some(fp) if fp == "NOCERTCHECK" => client_config_with_verifier(Arc::new(NoCertVerifier))?,
        Some(fp) => client_config_with_verifier(Arc::new(FingerprintVerifier {
            expected: fp.clone(),
        }))?,
        None => default_client_config()?,
    };

    Ok(Some(Connector::Rustls(Arc::new(client_config))))
}

fn default_client_config() -> Result<ClientConfig, ExasolError> {
    let mut roots = RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    if !native.errors.is_empty() {
        tracing::warn!("loaded native cert store with errors: {:?}", native.errors);
    }
    for cert in native.certs {
        if let Err(err) = roots.add(cert) {
            tracing::warn!("skipping invalid native CA: {err}");
        }
    }
    Ok(ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

fn client_config_with_verifier(
    verifier: Arc<dyn ServerCertVerifier>,
) -> Result<ClientConfig, ExasolError> {
    Ok(ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth())
}

#[derive(Debug)]
struct NoCertVerifier;

impl ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        all_supported_signature_schemes()
    }
}

#[derive(Debug)]
struct FingerprintVerifier {
    expected: String,
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        verify_fingerprint(&self.expected, end_entity.as_ref())
            .map(|_| ServerCertVerified::assertion())
            .map_err(|err| rustls::Error::General(err.to_string()))
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        all_supported_signature_schemes()
    }
}

fn all_supported_signature_schemes() -> Vec<SignatureScheme> {
    vec![
        SignatureScheme::RSA_PKCS1_SHA256,
        SignatureScheme::RSA_PKCS1_SHA384,
        SignatureScheme::RSA_PKCS1_SHA512,
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::ECDSA_NISTP521_SHA512,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
        SignatureScheme::ED25519,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs1::EncodeRsaPublicKey;
    use rsa::{RsaPrivateKey, RsaPublicKey};

    fn rsa_keypair() -> (RsaPrivateKey, RsaPublicKey) {
        let mut rng = OsRng;
        let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
        let public = RsaPublicKey::from(&private);
        (private, public)
    }

    #[test]
    fn encrypt_password_round_trips() {
        let (private, public) = rsa_keypair();
        let pem = public.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let original_password = "hunter2-secret";

        let ciphertext_b64 =
            super::encrypt_password(&pem, original_password).expect("encrypt_password");
        let ciphertext = BASE64.decode(ciphertext_b64).expect("base64 decode");
        let decrypted = private
            .decrypt(Pkcs1v15Encrypt, &ciphertext)
            .expect("decrypt with private key");

        assert_eq!(decrypted.as_slice(), original_password.as_bytes());
    }

    #[test]
    fn verify_fingerprint_accepts_matching_hash() {
        let cert = b"some-cert-der-bytes";
        let expected = certificate_sha256_hex(cert);

        super::verify_fingerprint(&expected, cert).expect("matching fingerprint accepted");
    }

    #[test]
    fn verify_fingerprint_accepts_lowercase_input() {
        let cert = b"some-cert-der-bytes";
        let expected = certificate_sha256_hex(cert).to_ascii_lowercase();

        super::verify_fingerprint(&expected, cert)
            .expect("lowercase fingerprint must be normalised before comparison");
    }

    #[test]
    fn verify_fingerprint_rejects_mismatch() {
        let cert = b"some-cert-der-bytes";
        let wrong = "DEADBEEF".repeat(8);

        let err = super::verify_fingerprint(&wrong, cert).expect_err("mismatched fingerprint");
        match err {
            ExasolError::Connection(msg) => assert!(msg.contains("fingerprint mismatch")),
            other => panic!("expected Connection error, got {other:?}"),
        }
    }

    #[test]
    fn certificate_sha256_hex_is_uppercase() {
        let cert = b"another-blob";
        let hex = certificate_sha256_hex(cert);

        assert_eq!(hex, hex.to_ascii_uppercase());
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn parse_result_set_extracts_typed_rows() {
        let result = json!({
            "resultType": "resultSet",
            "resultSet": {
                "numColumns": 2,
                "numRows": 2,
                "numRowsInMessage": 2,
                "columns": [
                    { "name": "id", "dataType": { "type": "DECIMAL", "precision": 18, "scale": 0 } },
                    { "name": "name", "dataType": { "type": "VARCHAR", "size": 100 } },
                ],
                "data": [[1, 2], ["alice", "bob"]],
            },
        });

        let result_set = result.get("resultSet").unwrap();
        let columns = parse_columns(result_set).expect("parse_columns");
        let rows = transpose_data(result_set.get("data"));

        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].name, "id");
        assert_eq!(columns[1].name, "name");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec![Some("1".into()), Some("alice".into())]);
        assert_eq!(rows[1], vec![Some("2".into()), Some("bob".into())]);
    }

    #[test]
    fn transpose_data_handles_nulls_and_types() {
        let data = json!([[null, 42, true], ["x", null, false],]);

        let rows = transpose_data(Some(&data));

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], vec![None, Some("x".into())]);
        assert_eq!(rows[1], vec![Some("42".into()), None]);
        assert_eq!(rows[2], vec![Some("t".into()), Some("f".into())]);
    }

    #[test]
    fn value_to_text_handles_primitive_variants() {
        assert_eq!(value_to_text(None), None);
        assert_eq!(value_to_text(Some(&Value::Null)), None);
        assert_eq!(
            value_to_text(Some(&Value::String("foo".into()))),
            Some("foo".into())
        );
        assert_eq!(value_to_text(Some(&Value::Bool(true))), Some("t".into()));
        assert_eq!(value_to_text(Some(&Value::Bool(false))), Some("f".into()));
        assert_eq!(
            value_to_text(Some(&Value::Number(42i64.into()))),
            Some("42".into())
        );
    }

    #[test]
    fn response_text_from_message_extracts_text_frames() {
        let text = super::response_text_from_message(Message::Text("hello".to_owned())).unwrap();
        assert_eq!(text.as_deref(), Some("hello"));
    }

    #[test]
    fn response_text_from_message_skips_pong_progress() {
        let result =
            super::response_text_from_message(Message::Pong(b"EXECUTING".to_vec())).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn response_text_from_message_reports_close_as_error() {
        let err = super::response_text_from_message(Message::Close(None));
        assert!(matches!(err, Err(ExasolError::Request(_))));
    }
}
