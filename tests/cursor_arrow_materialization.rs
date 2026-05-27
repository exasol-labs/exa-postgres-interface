// Integration tests for cursor materialisation of Arrow RecordBatches.
// Verifies that DECLARE / FETCH cursor operations materialise Arrow data from
// Exasol and stream it correctly to the pgwire client via the GatewayCursor.
// Requires a live Exasol instance. Run with:
//   cargo test --test cursor_arrow_materialization -- --ignored

mod common;

use tokio_postgres::{NoTls, SimpleQueryMessage};

use common::{pg_connection_string, spawn_gateway};

#[tokio::test]
#[ignore = "live exasol"]
async fn declare_then_fetch_streams_record_batches_to_client() {
    let (addr, _gateway) = spawn_gateway().await;

    let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
        .await
        .expect("pgwire connect");
    let conn_handle = tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("tokio-postgres background connection ended: {error}");
        }
    });

    // BEGIN starts a transaction; DECLARE materialises the result into the
    // GatewayCursor as Arrow batches. Two FETCH 2 calls walk a flat row index
    // across whatever batch shape exarrow-rs produces, then CLOSE drops it.
    client
        .simple_query("BEGIN")
        .await
        .expect("BEGIN starts a transaction");

    client
        .simple_query(
            "DECLARE c CURSOR FOR \
             SELECT n FROM (VALUES (1), (2), (3), (4), (5)) AS v(n) ORDER BY n",
        )
        .await
        .expect("DECLARE cursor over a small VALUES list");

    let first_rows = fetch_n_values(&client, 2).await;
    assert_eq!(
        first_rows,
        vec!["1".to_owned(), "2".to_owned()],
        "first FETCH should yield the first two rows in order"
    );

    let second_rows = fetch_n_values(&client, 2).await;
    assert_eq!(
        second_rows,
        vec!["3".to_owned(), "4".to_owned()],
        "second FETCH should advance past the first two rows"
    );

    client.simple_query("CLOSE c").await.expect("CLOSE cursor");
    client
        .simple_query("COMMIT")
        .await
        .expect("COMMIT ends the transaction");

    drop(client);
    let _ = conn_handle.await;
}

/// Issue `FETCH n FROM c` over the simple-query protocol and return the
/// rendered column-0 values in receipt order.
async fn fetch_n_values(client: &tokio_postgres::Client, n: u32) -> Vec<String> {
    let messages = client
        .simple_query(&format!("FETCH {n} FROM c"))
        .await
        .unwrap_or_else(|err| panic!("FETCH {n}: {err}"));

    messages
        .iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(row) => row.get(0).map(|s| s.to_owned()),
            _ => None,
        })
        .collect()
}
