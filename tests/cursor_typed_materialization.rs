// Integration tests for cursor materialisation via the WebSocket (typed)
// transport. The Arrow path is covered by `cursor_arrow_materialization.rs`;
// this peer file proves that DECLARE / FETCH / CLOSE also work end-to-end
// when `ExasolOutcome::TypedRows` is fed into `GatewayCursor` instead of
// `ExasolOutcome::ArrowRows`. Requires a live Exasol instance. Run with:
//   cargo test --test cursor_typed_materialization -- --ignored

mod common;

use std::sync::Arc;

use tokio_postgres::{NoTls, SimpleQueryMessage};

use common::{TransportTestConfig, pg_connection_string, spawn_gateway_with_config};
use exa_postgres_interface::config::ExasolConfig;

#[tokio::test]
#[ignore = "live exasol"]
async fn declare_then_fetch_under_websocket_transport() {
    // BEGIN / DECLARE / FETCH / CLOSE / COMMIT over the simple-query protocol
    // against a gateway forced onto the WebSocket transport. The DECLARE
    // round-trips a single-row `SELECT 1` so the test exercises the typed
    // cursor-materialisation branch in `GatewayCursor` without depending on
    // any specific Arrow shape.
    let exasol = ExasolConfig {
        transport: "websocket".to_owned(),
        ..common::live_exasol_config()
    };
    let tcfg = TransportTestConfig {
        transport: "websocket",
        exasol_config: exasol,
    };

    let (addr, _gateway) = spawn_gateway_with_config(Arc::new(tcfg.app_config(0))).await;

    let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
        .await
        .expect("pgwire connect");
    let conn_handle = tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("tokio-postgres background connection ended: {error}");
        }
    });

    client
        .simple_query("BEGIN")
        .await
        .expect("BEGIN starts a transaction");

    client
        .simple_query("DECLARE c CURSOR FOR SELECT 1")
        .await
        .expect("DECLARE cursor over SELECT 1");

    let messages = client
        .simple_query("FETCH 1 FROM c")
        .await
        .expect("FETCH 1 succeeds");

    let rows: Vec<String> = messages
        .iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(row) => row.get(0).map(|s| s.to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(
        rows,
        vec!["1".to_owned()],
        "FETCH 1 should yield exactly one row containing the literal 1"
    );

    client.simple_query("CLOSE c").await.expect("CLOSE cursor");
    client
        .simple_query("COMMIT")
        .await
        .expect("COMMIT ends the transaction");

    drop(client);
    let _ = conn_handle.await;
}
