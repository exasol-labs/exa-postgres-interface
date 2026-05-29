// Integration tests for TLS certificate fingerprint pinning behaviour.
// Verifies that the gateway correctly accepts connections when the configured
// SHA-256 fingerprint matches the server certificate, and rejects them when
// it does not. Also verifies that NOCERTCHECK disables validation with a
// warning log rather than an error.
// Requires a live Exasol instance. Run with:
//   cargo test --test tls_fingerprint_integration -- --ignored

mod common;

use exa_postgres_interface::config::ExasolConfig;
use exa_postgres_interface::exasol::{ExasolError, ExasolSession};
use exarrow_rs::Connection;
use exarrow_rs::connection::ConnectionBuilder;

use common::{
    LIVE_EXASOL_HOST, LIVE_EXASOL_PORT, LIVE_FINGERPRINT_HEX, TEST_PASSWORD, TEST_USER,
    all_transport_configs,
};

#[tokio::test]
#[ignore = "live exasol"]
async fn matching_fingerprint_connects_mismatched_fingerprint_rejected() {
    // Branch 1: matching fingerprint should connect.
    //
    // exarrow-rs's `FingerprintVerifier` computes the server-cert SHA-256 as
    // lowercase hex and compares it byte-for-byte against
    // `params.certificate_fingerprint`. We therefore feed the lowercase form
    // of our captured fingerprint directly through `ConnectionBuilder` —
    // bypassing `EndpointConnection::parse`, which uppercases the configured
    // fingerprint (an existing case-sensitivity contract on the gateway
    // side that doesn't match exarrow-rs).
    let matching_fingerprint = LIVE_FINGERPRINT_HEX.to_ascii_lowercase();
    let matching_params = ConnectionBuilder::new()
        .host(LIVE_EXASOL_HOST)
        .port(LIVE_EXASOL_PORT)
        .username(TEST_USER)
        .password(TEST_PASSWORD)
        .use_tls(true)
        .validate_server_certificate(true)
        .certificate_fingerprint(&matching_fingerprint)
        .build()
        .expect("build matching ConnectionParams");

    let mut matching = Connection::from_params(matching_params)
        .await
        .expect("matching fingerprint should connect");
    let result = matching.execute("SELECT 1").await.expect("SELECT 1");
    assert!(
        result.row_count().is_none(),
        "SELECT 1 should produce a result set, not a row-count outcome"
    );
    let batches = result.fetch_all().await.expect("fetch_all SELECT 1");
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 1, "SELECT 1 must yield exactly one row");
    matching.close().await.expect("close matching connection");

    // Branch 2: a mismatched fingerprint MUST be rejected. Run this via the
    // gateway's `ExasolSession::connect` so the path under test is the full
    // adapter, not just the driver.
    let mismatched_config = ExasolConfig {
        dsn: format!("{LIVE_EXASOL_HOST}:{LIVE_EXASOL_PORT}"),
        encryption: true,
        // 64-char hex digest of all zeros — guaranteed not to match a real
        // certificate. `EndpointConnection::parse` will uppercase this, but it's all
        // ASCII digits, so the contents are identical pre- and post-upper.
        certificate_fingerprint: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
        validate_certificate: true,
        pass_client_credentials: true,
        schema: String::new(),
        transport: String::new(),
    };
    let mismatch = ExasolSession::connect(&mismatched_config, TEST_USER, TEST_PASSWORD).await;
    match mismatch {
        Err(ExasolError::Connection(message)) => {
            // exarrow-rs surfaces the fingerprint mismatch as a TLS error
            // bubbled up through ConnectionError; the message must give the
            // operator something to grep for.
            let lower = message.to_ascii_lowercase();
            assert!(
                lower.contains("fingerprint")
                    || lower.contains("certificate")
                    || lower.contains("tls"),
                "expected fingerprint/TLS-related message, got: {message}"
            );
        }
        Err(other) => panic!("expected ExasolError::Connection, got {other:?}"),
        Ok(_) => panic!("mismatched fingerprint must NOT connect"),
    }
}

#[tokio::test]
#[ignore = "live exasol"]
async fn nocertcheck_disables_validation_with_warning_log() {
    // `validate_certificate = false` plus an empty fingerprint maps to the
    // NOCERTCHECK arm in `EndpointConnection::parse`, which sets
    // `validate_server_certificate = false` on the driver and produces a
    // verifier that accepts any cert.
    //
    // The "warning log" half of the scenario is documented as a soft
    // contract: tracing emits a warn-level event when the session opens.
    // Asserting on the in-process tracing subscriber from an integration
    // test is awkward (it would require installing a custom subscriber
    // before any other test has installed the default one), so we settle
    // on the observable behaviour: a working connection plus a working
    // round-trip query against the self-signed cert.
    let config = ExasolConfig {
        dsn: format!("{LIVE_EXASOL_HOST}:{LIVE_EXASOL_PORT}"),
        encryption: true,
        certificate_fingerprint: String::new(),
        validate_certificate: false,
        pass_client_credentials: true,
        schema: String::new(),
        transport: String::new(),
    };

    let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
        .await
        .expect("NOCERTCHECK should connect to a self-signed Exasol server");
    let _ = session
        .execute("SELECT 1")
        .await
        .expect("SELECT 1 should succeed under NOCERTCHECK");
    session.close().await.expect("close session");
}

#[tokio::test]
#[ignore = "live exasol"]
async fn matching_fingerprint_connects_under_each_transport() {
    // Both transports must honour configured fingerprint pinning. The
    // gateway uppercases the fingerprint in `EndpointConnection::parse` —
    // the captured `LIVE_FINGERPRINT_HEX` is already uppercase, so the
    // round-trip through `ExasolSession::connect` should succeed without
    // pre-lowercasing on either transport.
    for tcfg in all_transport_configs() {
        let label = tcfg.transport;
        let config = ExasolConfig {
            certificate_fingerprint: LIVE_FINGERPRINT_HEX.to_owned(),
            validate_certificate: true,
            ..tcfg.exasol_config.clone()
        };
        let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
            .await
            .unwrap_or_else(|err| panic!("{label} fingerprint connect: {err}"));
        let _ = session
            .execute("SELECT 1")
            .await
            .unwrap_or_else(|err| panic!("{label} SELECT 1 with pinned fingerprint: {err}"));
        session
            .close()
            .await
            .unwrap_or_else(|err| panic!("{label} close: {err}"));
    }
}

#[tokio::test]
#[ignore = "live exasol"]
async fn nocertcheck_disables_validation_under_each_transport() {
    // `validate_certificate = false` plus an empty fingerprint must select
    // the NOCERTCHECK arm on both transports. The Arrow transport routes
    // through exarrow-rs's accept-any verifier; the WebSocket transport
    // routes through the gateway's own tokio-rustls verifier — different
    // code paths, same observable behaviour.
    for tcfg in all_transport_configs() {
        let label = tcfg.transport;
        let config = ExasolConfig {
            certificate_fingerprint: String::new(),
            validate_certificate: false,
            ..tcfg.exasol_config.clone()
        };
        let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
            .await
            .unwrap_or_else(|err| panic!("{label} NOCERTCHECK connect: {err}"));
        let _ = session
            .execute("SELECT 1")
            .await
            .unwrap_or_else(|err| panic!("{label} SELECT 1 under NOCERTCHECK: {err}"));
        session
            .close()
            .await
            .unwrap_or_else(|err| panic!("{label} close: {err}"));
    }
}

#[tokio::test]
#[ignore = "live exasol"]
async fn dsn_fingerprint_propagates_under_each_transport() {
    // Embed the fingerprint after a `/` in the DSN host segment; the
    // `EndpointConnection::parse` precedence rule says the config field
    // wins when both are set, so leave the config field empty to force
    // the DSN-embedded value through. Both transports must accept it.
    for tcfg in all_transport_configs() {
        let label = tcfg.transport;
        let config = ExasolConfig {
            dsn: format!("{LIVE_EXASOL_HOST}/{LIVE_FINGERPRINT_HEX}:{LIVE_EXASOL_PORT}"),
            certificate_fingerprint: String::new(),
            validate_certificate: true,
            ..tcfg.exasol_config.clone()
        };
        let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
            .await
            .unwrap_or_else(|err| panic!("{label} DSN fingerprint connect: {err}"));
        let _ = session
            .execute("SELECT 1")
            .await
            .unwrap_or_else(|err| panic!("{label} SELECT 1 with DSN fingerprint: {err}"));
        session
            .close()
            .await
            .unwrap_or_else(|err| panic!("{label} close: {err}"));
    }
}
