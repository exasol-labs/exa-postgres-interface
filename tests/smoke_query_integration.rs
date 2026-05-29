// Integration smoke test: verifies that a simple SELECT travels end-to-end
// from pgwire client through the gateway to Exasol and back as Arrow data.
// Requires a live Exasol instance. Run with:
//   cargo test --test smoke_query_integration -- --ignored

mod common;

use std::sync::Arc;

use tokio_postgres::{NoTls, SimpleQueryMessage};

use common::{
    all_transport_configs, pg_connection_string, spawn_gateway, spawn_gateway_with_config,
};

#[tokio::test]
#[ignore = "live exasol"]
async fn select_one_round_trips_arrow_through_pgwire() {
    let (addr, _gateway) = spawn_gateway().await;

    let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
        .await
        .expect("pgwire connect");
    let conn_handle = tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("tokio-postgres background connection ended: {error}");
        }
    });

    // Use the simple query protocol: text-only, no client-side type negotiation.
    // That matches what the gateway emits (FieldFormat::Text in every FieldInfo
    // produced by `map_exasol_columns`), so we can compare on the literal text
    // representation that an Arrow `Int32Array` would render through pgwire.
    let messages = client
        .simple_query("SELECT 1 AS x")
        .await
        .expect("SELECT 1 succeeds");

    let rows: Vec<_> = messages
        .iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(r) => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(rows.len(), 1, "SELECT 1 returns exactly one row");

    let row = rows[0];
    assert_eq!(row.len(), 1, "SELECT 1 returns exactly one column");

    let value = row.get(0).expect("x column is non-null");
    assert_eq!(value, "1", "SELECT 1 yields the literal integer 1");

    drop(client);
    let _ = conn_handle.await;
}

#[tokio::test]
#[ignore = "live exasol"]
async fn select_one_round_trips_under_each_transport() {
    // Exercise the gateway end-to-end (pgwire client -> ExasolSession ->
    // Exasol) under each configured transport. The text-protocol render of
    // `SELECT 1` must be the literal `"1"` regardless of whether the row is
    // sourced from an Arrow `RecordBatch` or a typed JSON row vector.
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

        let messages = client
            .simple_query("SELECT 1 AS x")
            .await
            .unwrap_or_else(|err| panic!("{label} SELECT 1: {err}"));

        let rows: Vec<_> = messages
            .iter()
            .filter_map(|m| match m {
                SimpleQueryMessage::Row(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(rows.len(), 1, "{label}: SELECT 1 returns exactly one row");

        let row = rows[0];
        assert_eq!(row.len(), 1, "{label}: SELECT 1 returns exactly one column");

        let value = row.get(0).expect("x column is non-null");
        assert_eq!(value, "1", "{label}: SELECT 1 yields the literal integer 1");

        drop(client);
        let _ = conn_handle.await;
    }
}
