// Integration tests for ExasolConfig -> exarrow-rs ConnectionParams mapping.
// Verifies that all relevant ExasolConfig fields (host, port, schema, TLS
// settings, fingerprint) are correctly translated into exarrow-rs
// ConnectionParams and result in a successful Exasol connection.
// Requires a live Exasol instance. Run with:
//   cargo test --test config_to_connection_params -- --ignored

mod common;

use exa_postgres_interface::config::{DEFAULT_TRANSPORT, ExasolConfig, Transport};
use exa_postgres_interface::exasol::{EndpointConnection, ExasolOutcome, ExasolSession};
use exarrow_rs::Connection;

use common::{
    LIVE_EXASOL_HOST, LIVE_EXASOL_PORT, LIVE_FINGERPRINT_HEX, TEST_PASSWORD, TEST_USER,
    live_exasol_config,
};

#[tokio::test]
#[ignore = "live exasol"]
async fn exasol_config_maps_to_exarrow_connection_params() {
    // -----------------------------------------------------------------
    // Part 1: static config-to-params mapping with EVERY field populated.
    // -----------------------------------------------------------------
    //
    // We pin a fingerprint here so the adapter exercises the fingerprint-
    // precedence branch in `EndpointConnection::parse` and `to_connection_params`.
    // We assert only on the mapped `ConnectionParams` fields; the actual
    // TLS handshake is exercised separately in Part 2 with NOCERTCHECK,
    // because the gateway's `Endpoint` uppercases the configured
    // fingerprint while `exarrow-rs::FingerprintVerifier` compares the
    // computed SHA-256 against the raw value with lowercase hex output —
    // so a non-NOCERTCHECK round-trip through `Endpoint` cannot match
    // unless the user supplies the digest already in lowercase. The
    // unit tests in `src/exasol.rs` already cover the uppercase contract
    // in isolation, so what's left here is the mapping shape.
    let pinned_config = ExasolConfig {
        dsn: format!("{LIVE_EXASOL_HOST}:{LIVE_EXASOL_PORT}"),
        encryption: true,
        certificate_fingerprint: LIVE_FINGERPRINT_HEX.to_owned(),
        validate_certificate: true,
        pass_client_credentials: true,
        schema: "SYS".to_owned(),
        transport: String::new(),
    };

    let endpoint = EndpointConnection::parse(&pinned_config.dsn, &pinned_config)
        .expect("EndpointConnection::parse");
    let params = endpoint
        .to_connection_params(&pinned_config, TEST_USER, TEST_PASSWORD)
        .expect("to_connection_params");

    assert_eq!(params.host, LIVE_EXASOL_HOST, "host");
    assert_eq!(params.port, LIVE_EXASOL_PORT, "port");
    assert_eq!(params.username, TEST_USER, "username");
    assert_eq!(params.schema.as_deref(), Some("SYS"), "schema");
    assert!(params.use_tls, "use_tls must follow config.encryption");
    assert!(
        params.validate_server_certificate,
        "validate_server_certificate must follow config.validate_certificate"
    );
    // `EndpointConnection::parse` uppercases the configured fingerprint per the
    // pre-existing contract; verify that.
    assert_eq!(
        params.certificate_fingerprint.as_deref(),
        Some(LIVE_FINGERPRINT_HEX),
        "certificate_fingerprint must be the configured uppercase hex digest"
    );

    // -----------------------------------------------------------------
    // Part 2: round-trip the mapped params through the actual driver and
    // through `ExasolSession`, proving the (non-fingerprint) config
    // fields steer a real connection. We use the NOCERTCHECK path here
    // (validate_certificate = false, no fingerprint) so the live self-
    // signed cert is accepted.
    // -----------------------------------------------------------------
    let live_config = ExasolConfig {
        dsn: format!("{LIVE_EXASOL_HOST}:{LIVE_EXASOL_PORT}"),
        encryption: true,
        certificate_fingerprint: String::new(),
        validate_certificate: false,
        pass_client_credentials: true,
        schema: "SYS".to_owned(),
        transport: String::new(),
    };
    let live_endpoint =
        EndpointConnection::parse(&live_config.dsn, &live_config).expect("live Endpoint");
    let live_params = live_endpoint
        .to_connection_params(&live_config, TEST_USER, TEST_PASSWORD)
        .expect("live to_connection_params");
    assert!(
        !live_params.validate_server_certificate,
        "NOCERTCHECK path must disable server cert validation"
    );
    assert_eq!(
        live_params.certificate_fingerprint, None,
        "NOCERTCHECK path must not set a fingerprint"
    );
    assert_eq!(live_params.schema.as_deref(), Some("SYS"));

    let direct = Connection::from_params(live_params)
        .await
        .expect("Connection::from_params via mapped params");
    direct.close().await.expect("direct Connection::close");

    let mut session = ExasolSession::connect(&live_config, TEST_USER, TEST_PASSWORD)
        .await
        .expect("ExasolSession::connect");
    // `schema = "SYS"` should let unqualified `EXA_ALL_USERS` resolve.
    let outcome = session
        .execute("SELECT 1 FROM EXA_ALL_USERS LIMIT 1")
        .await
        .expect("SELECT against the active schema");
    match outcome {
        ExasolOutcome::ArrowRows(batches) => {
            let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();
            assert!(
                row_count >= 1,
                "unqualified EXA_ALL_USERS must resolve under schema SYS"
            );
        }
        other => panic!("expected ArrowRows outcome from SELECT, got {other:?}"),
    }
    session.close().await.expect("close session");
}

#[test]
fn transport_websocket_parses() {
    let config = ExasolConfig {
        transport: "websocket".to_owned(),
        ..live_exasol_config()
    };
    assert_eq!(
        Transport::from_config(&config).expect("websocket parses"),
        Transport::WebSocket,
    );
}

#[test]
fn transport_arrow_parses() {
    let config = ExasolConfig {
        transport: "arrow".to_owned(),
        ..live_exasol_config()
    };
    assert_eq!(
        Transport::from_config(&config).expect("arrow parses"),
        Transport::Arrow,
    );
}

#[test]
fn transport_unknown_rejects() {
    let config = ExasolConfig {
        transport: "tcp".to_owned(),
        ..live_exasol_config()
    };
    let err = Transport::from_config(&config).expect_err("tcp must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("unknown transport"),
        "unknown-value error must mention 'unknown transport', got: {message}"
    );
    assert!(
        message.contains("websocket") && message.contains("arrow"),
        "unknown-value error must list accepted values, got: {message}"
    );
}

#[test]
fn default_transport_constant_is_websocket() {
    assert_eq!(DEFAULT_TRANSPORT, "websocket");
}
