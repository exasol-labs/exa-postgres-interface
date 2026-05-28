// Integration tests for transport selection via configuration.
// These tests require a live Exasol instance.
// Run with: cargo test --test transport_selection -- --ignored

mod common;

use exa_postgres_interface::config::{DEFAULT_TRANSPORT, ExasolConfig, Transport};
use exa_postgres_interface::exasol::{ExasolOutcome, ExasolSession};

use common::{TEST_PASSWORD, TEST_USER, live_exasol_config_for_transport};

#[test]
fn default_transport_is_arrow() {
    assert_eq!(DEFAULT_TRANSPORT, "arrow");
}

#[test]
fn transport_choice_is_fixed_for_session_lifetime() {
    // The transport is selected at connect time and does not change mid-session.
    // This is a compile-time invariant (`Box<dyn ExasolTransport>` is fixed at
    // connect), so we assert the configuration-level side of the contract:
    // `DEFAULT_TRANSPORT == "arrow"` and `Transport::from_config` parses
    // it deterministically.
    let config = ExasolConfig {
        transport: DEFAULT_TRANSPORT.to_owned(),
        ..common::live_exasol_config()
    };
    assert_eq!(
        Transport::from_config(&config).expect("default transport parses"),
        Transport::Arrow
    );
}

#[tokio::test]
#[ignore = "live exasol"]
async fn explicit_websocket_transport_runs_websocket_path() {
    let config = live_exasol_config_for_transport("websocket");
    let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
        .await
        .expect("websocket connect");
    let outcome = session.execute("SELECT 1").await.expect("execute");
    assert!(
        matches!(outcome, ExasolOutcome::TypedRows { .. }),
        "websocket transport must return TypedRows, got {outcome:?}"
    );
    session.close().await.expect("close");
}

#[tokio::test]
#[ignore = "live exasol"]
async fn explicit_arrow_transport_runs_arrow_path() {
    let config = live_exasol_config_for_transport("arrow");
    let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
        .await
        .expect("arrow connect");
    let outcome = session.execute("SELECT 1").await.expect("execute");
    assert!(
        matches!(outcome, ExasolOutcome::ArrowRows(_)),
        "arrow transport must return ArrowRows, got {outcome:?}"
    );
    session.close().await.expect("close");
}

#[test]
fn unknown_transport_value_is_rejected() {
    let config = ExasolConfig {
        transport: "tcp".to_owned(),
        ..common::live_exasol_config()
    };
    let err = Transport::from_config(&config).expect_err("tcp is not a transport");
    let msg = err.to_string();
    assert!(
        msg.contains("unknown transport") && msg.contains("websocket") && msg.contains("arrow"),
        "rejection message must enumerate valid values, got: {msg}"
    );
}
