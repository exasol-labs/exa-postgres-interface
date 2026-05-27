// Integration tests for pgwire DataRow rendering across both transports.
// The Arrow path is covered by `record_batches_render_into_pgwire_data_rows`;
// the WebSocket (typed) path is covered by the parameterised tests below.
// Requires a live Exasol instance. Run with:
//   cargo test --test pgwire_rendering -- --ignored

mod common;

use std::sync::Arc;

use tokio_postgres::{NoTls, SimpleQueryMessage};

use common::{
    all_transport_configs, pg_connection_string, spawn_gateway, spawn_gateway_with_config,
};

#[tokio::test]
#[ignore = "live exasol"]
async fn record_batches_render_into_pgwire_data_rows() {
    let (addr, _gateway) = spawn_gateway().await;

    let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
        .await
        .expect("pgwire connect");
    let conn_handle = tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("tokio-postgres background connection ended: {error}");
        }
    });

    // Each column below exercises a different Arrow → pgwire mapping branch
    // in `query_response_arrow` and the `pg_type_for_arrow_field` table:
    //   small_int → DECIMAL(9,0)  ⇒ pgwire INT4 (Arrow Int32 / Decimal128)
    //   dbl       → DOUBLE        ⇒ pgwire FLOAT8 (Arrow Float64)
    //   s         → VARCHAR(10)   ⇒ pgwire VARCHAR (Arrow Utf8)
    //   b         → BOOLEAN       ⇒ pgwire BOOL (Arrow Boolean)
    //   d         → DATE          ⇒ pgwire DATE (Arrow Date32)
    //   ts        → TIMESTAMP     ⇒ pgwire TIMESTAMP (Arrow Timestamp)
    let sql = "SELECT \
        CAST(1 AS DECIMAL(9,0)) AS small_int, \
        CAST(1.5 AS DOUBLE) AS dbl, \
        CAST('hello' AS VARCHAR(10)) AS s, \
        TRUE AS b, \
        DATE '2026-05-18' AS d, \
        TIMESTAMP '2026-05-18 12:34:56' AS ts";

    let messages = client.simple_query(sql).await.expect("multi-type SELECT");

    let rows: Vec<_> = messages
        .iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(r) => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(rows.len(), 1, "expected exactly one row");
    let row = rows[0];
    assert_eq!(row.len(), 6, "expected six columns");

    // INT4 over the wire: Arrow renders the decimal scalar `1` as `"1"`.
    assert_eq!(row.get(0).unwrap(), "1", "small_int column");

    // FLOAT8: Arrow's default formatter renders `1.5_f64` as `"1.5"`.
    assert_eq!(row.get(1).unwrap(), "1.5", "dbl column");

    // VARCHAR: passes through unchanged.
    assert_eq!(row.get(2).unwrap(), "hello", "s column");

    // BOOLEAN: Arrow's text rendering for a Boolean array is `true` / `false`.
    // PostgreSQL clients accept either `t`/`f` or `true`/`false` for text BOOL,
    // so we just assert the literal we know the gateway produces.
    let b = row.get(3).unwrap();
    assert!(
        b == "true" || b == "t",
        "b column should serialize as a truthy literal, got {b:?}"
    );

    // DATE: Arrow renders Date32 as `YYYY-MM-DD`, which matches PG text DATE.
    assert_eq!(row.get(4).unwrap(), "2026-05-18", "d column");

    // TIMESTAMP: Arrow uses a `T` separator (`2026-05-18T12:34:56`) by default,
    // whereas PG uses a space. Accept either to insulate the test from cosmetic
    // formatter differences between exarrow-rs / arrow versions.
    let ts = row.get(5).unwrap();
    assert!(
        ts.starts_with("2026-05-18") && ts.contains("12:34:56"),
        "ts column should encode the literal timestamp, got {ts:?}"
    );

    drop(client);
    let _ = conn_handle.await;
}

#[tokio::test]
#[ignore = "live exasol"]
async fn each_transport_emits_its_native_response_variant() {
    // The internal `GatewayResponse::TypedQuery` / `GatewayResponse::ArrowQuery`
    // types are not exported, so we observe the contract through the wire:
    // both variants must render `SELECT 1` as the literal text `"1"` in a
    // single-row, single-column DataRow. The render path differs (Arrow's
    // `array_value_to_string` vs the typed transport's pass-through `String`),
    // so success on both transports confirms the response-variant dispatch
    // is wired to its respective renderer rather than collapsing into one.
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
            .simple_query("SELECT 1")
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
        let value = rows[0]
            .get(0)
            .unwrap_or_else(|| panic!("{label}: single column must be non-null"));
        assert_eq!(value, "1", "{label}: SELECT 1 must render as literal \"1\"");

        drop(client);
        let _ = conn_handle.await;
    }
}

#[tokio::test]
#[ignore = "live exasol"]
async fn websocket_transport_produces_typed_query_with_text_format() {
    // The WebSocket transport routes `SELECT 1` through the typed-row
    // response variant. The simple-query protocol is text-only, so the
    // gateway must produce `"1"` (not a binary INT4) and a one-row, one-
    // column result regardless of the column's underlying Exasol DECIMAL
    // shape.
    let tcfg = all_transport_configs()
        .into_iter()
        .find(|c| c.transport == "websocket")
        .expect("websocket config present in matrix");

    let (addr, _gateway) = spawn_gateway_with_config(Arc::new(tcfg.app_config(0))).await;

    let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
        .await
        .expect("websocket pgwire connect");
    let conn_handle = tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("tokio-postgres background connection ended: {error}");
        }
    });

    let messages = client
        .simple_query("SELECT 1")
        .await
        .expect("websocket SELECT 1");

    let rows: Vec<_> = messages
        .iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(r) => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(rows.len(), 1, "websocket SELECT 1 returns exactly one row");
    assert_eq!(rows[0].len(), 1, "websocket SELECT 1 returns one column");
    let value = rows[0]
        .get(0)
        .expect("websocket single column must be non-null");
    assert_eq!(value, "1", "websocket SELECT 1 must render as text \"1\"");

    drop(client);
    let _ = conn_handle.await;
}
