// Integration tests for SET search_path error propagation via the gateway.
// Verifies that setting search_path to a schema that does not exist in Exasol
// results in a PostgreSQL-compatible error response to the client.
// Requires a live Exasol instance. Run with:
//   cargo test --test search_path_integration -- --ignored

mod common;

use std::sync::Arc;

use tokio_postgres::NoTls;

use common::{
    all_transport_configs, pg_connection_string, spawn_gateway, spawn_gateway_with_config,
};

#[tokio::test]
#[ignore = "live exasol"]
async fn set_search_path_to_missing_schema_returns_pg_error() {
    let (addr, _gateway) = spawn_gateway().await;

    let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
        .await
        .expect("pgwire connect");
    let conn_handle = tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("tokio-postgres background connection ended: {error}");
        }
    });

    let result = client
        .simple_query("SET search_path TO \"definitely_not_a_real_schema_xyz123\"")
        .await;

    let err = result.expect_err(
        "SET search_path to a missing schema must surface a PostgreSQL error from the gateway",
    );
    let db_error = err
        .as_db_error()
        .expect("the gateway must surface a structured DbError, not a transport error");

    let sqlstate = db_error.code().code();
    assert!(
        !sqlstate.is_empty(),
        "the gateway must populate SQLSTATE on the error response"
    );

    // The error should be a 5-character SQLSTATE per the PostgreSQL protocol.
    // `map_exasol_execution_error` currently labels every Exasol-side failure
    // as XX000 (internal_error), so we accept either that or the
    // semantically narrower 3F000 (invalid_schema_name) — whichever the
    // gateway settles on. What matters for this test is that the error is
    // PostgreSQL-shaped, not silently swallowed.
    assert_eq!(
        sqlstate.len(),
        5,
        "SQLSTATE must be 5 characters, got {sqlstate:?}"
    );
    assert!(
        sqlstate == "XX000" || sqlstate == "3F000",
        "expected gateway SQLSTATE XX000 or 3F000, got {sqlstate}: {}",
        db_error.message()
    );

    // The message should mention the offending schema name so callers can
    // diagnose the failure. Exasol returns an error referencing the schema.
    let message = db_error.message();
    assert!(
        message
            .to_ascii_lowercase()
            .contains("definitely_not_a_real_schema_xyz123")
            || message.to_ascii_lowercase().contains("schema"),
        "error message should reference the missing schema, got {message}"
    );

    drop(client);
    let _ = conn_handle.await;
}

#[tokio::test]
#[ignore = "live exasol"]
async fn set_search_path_to_missing_schema_returns_pg_error_under_each_transport() {
    // Both transports must funnel Exasol's "schema not found" diagnostic into
    // a structured pgwire DbError with a 5-char SQLSTATE. The error-mapping
    // code is shared between transports, but the surface that originates the
    // error differs (Arrow driver vs the WebSocket JSON `exception` field),
    // so we have to verify each.
    for tcfg in all_transport_configs() {
        let label = tcfg.transport;
        let (addr, _gateway) = spawn_gateway_with_config(Arc::new(tcfg.app_config(0))).await;

        let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
            .await
            .unwrap_or_else(|err| panic!("{label} pgwire connect: {err}"));
        let conn_handle = tokio::spawn(async move {
            if let Err(error) = connection.await {
                eprintln!("tokio-postgres background connection ended: {error}");
            }
        });

        let result = client
            .simple_query("SET search_path TO \"definitely_not_a_real_schema_xyz123\"")
            .await;
        let err = match result {
            Err(err) => err,
            Ok(_) => panic!(
                "{label}: SET search_path to a missing schema must surface a PostgreSQL error"
            ),
        };
        let db_error = err.as_db_error().unwrap_or_else(|| {
            panic!("{label}: the gateway must surface a structured DbError, not a transport error")
        });

        let sqlstate = db_error.code().code();
        assert_eq!(
            sqlstate.len(),
            5,
            "{label}: SQLSTATE must be 5 characters, got {sqlstate:?}"
        );
        assert!(
            sqlstate == "XX000" || sqlstate == "3F000",
            "{label}: expected gateway SQLSTATE XX000 or 3F000, got {sqlstate}: {}",
            db_error.message()
        );

        let message = db_error.message().to_ascii_lowercase();
        assert!(
            message.contains("definitely_not_a_real_schema_xyz123") || message.contains("schema"),
            "{label}: error message should reference the missing schema, got {message}"
        );

        drop(client);
        let _ = conn_handle.await;
    }
}
