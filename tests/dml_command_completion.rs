// Integration tests for DML command-completion through the Arrow outcome path.
// Verifies that DML statements executed via the gateway return the correct
// pgwire command-completion tag carrying the Exasol row count.
// Requires a live Exasol instance. Run with:
//   cargo test --test dml_command_completion -- --ignored

mod common;

use std::sync::Arc;

use tokio_postgres::{NoTls, SimpleQueryMessage};

use common::{
    all_transport_configs, pg_connection_string, spawn_gateway, spawn_gateway_with_config,
};

/// Unique-per-test schema name. Tests rely on the live setup user (`sys`)
/// having permission to create and drop schemas under arbitrary names.
const TEST_SCHEMA: &str = "exa_pg_test_dml";

#[tokio::test]
#[ignore = "live exasol"]
async fn update_returns_exasol_row_count_through_arrow_outcome() {
    let (addr, _gateway) = spawn_gateway().await;

    let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
        .await
        .expect("pgwire connect");
    let conn_handle = tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("tokio-postgres background connection ended: {error}");
        }
    });

    // Drop any leftover schema from a previously aborted run so the test is
    // safe to re-run. Ignore failures (schema may not exist yet).
    let _ = client
        .simple_query(&format!("DROP SCHEMA \"{TEST_SCHEMA}\" CASCADE"))
        .await;

    // Set up the fixture: schema, table, three rows, commit so it's visible.
    client
        .simple_query(&format!("CREATE SCHEMA \"{TEST_SCHEMA}\""))
        .await
        .expect("CREATE SCHEMA");
    client
        .simple_query(&format!(
            "CREATE TABLE \"{TEST_SCHEMA}\".\"t\" (k DECIMAL(9,0), v DECIMAL(9,0))"
        ))
        .await
        .expect("CREATE TABLE");
    client
        .simple_query(&format!(
            "INSERT INTO \"{TEST_SCHEMA}\".\"t\" VALUES (1, 10), (2, 20), (3, 30)"
        ))
        .await
        .expect("INSERT initial rows");
    client.simple_query("COMMIT").await.expect("commit fixture");

    let update_sql = format!("UPDATE \"{TEST_SCHEMA}\".\"t\" SET v = v + 1 WHERE k IN (1, 2)");

    // simple_query returns a CommandComplete message per statement. For
    // `UPDATE`, the row count rides along in the message.
    let messages = client
        .simple_query(&update_sql)
        .await
        .expect("UPDATE statement");

    let updated_rows = messages
        .iter()
        .find_map(|m| match m {
            SimpleQueryMessage::CommandComplete(n) => Some(*n),
            _ => None,
        })
        .expect("UPDATE should produce a CommandComplete");

    assert_eq!(
        updated_rows, 2,
        "UPDATE on k IN (1,2) should affect exactly two rows, got {updated_rows}"
    );

    // Tear down so the test is repeatable.
    client
        .simple_query(&format!("DROP SCHEMA \"{TEST_SCHEMA}\" CASCADE"))
        .await
        .expect("DROP SCHEMA");

    drop(client);
    let _ = conn_handle.await;
}

#[tokio::test]
#[ignore = "live exasol"]
async fn update_returns_row_count_under_each_transport() {
    // Run an UPDATE round-trip through the gateway once per transport. The
    // gateway must surface the Exasol row count as the pgwire
    // CommandComplete tag (`UPDATE N`) on both the Arrow and the WebSocket
    // path — the typed-rows path uses a different `ExasolOutcome` variant
    // (`RowCount(i64)`) and a separate `map_exasol_result` branch, so we
    // can't take that for granted.
    for tcfg in all_transport_configs() {
        let label = tcfg.transport;
        // Per-transport schema name so concurrent transport iterations don't
        // collide on the fixture even if the test runner ever runs them in
        // parallel.
        let schema = format!("{TEST_SCHEMA}_{label}");

        let (addr, _gateway) = spawn_gateway_with_config(Arc::new(tcfg.app_config(0))).await;

        let (client, connection) = tokio_postgres::connect(&pg_connection_string(addr), NoTls)
            .await
            .unwrap_or_else(|err| panic!("{label} pgwire connect: {err}"));
        let conn_handle = tokio::spawn(async move {
            if let Err(error) = connection.await {
                eprintln!("tokio-postgres background connection ended: {error}");
            }
        });

        let _ = client
            .simple_query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
            .await;

        client
            .simple_query(&format!("CREATE SCHEMA \"{schema}\""))
            .await
            .unwrap_or_else(|err| panic!("{label} CREATE SCHEMA: {err}"));
        client
            .simple_query(&format!(
                "CREATE TABLE \"{schema}\".\"t\" (k DECIMAL(9,0), v DECIMAL(9,0))"
            ))
            .await
            .unwrap_or_else(|err| panic!("{label} CREATE TABLE: {err}"));
        client
            .simple_query(&format!(
                "INSERT INTO \"{schema}\".\"t\" VALUES (1, 10), (2, 20), (3, 30)"
            ))
            .await
            .unwrap_or_else(|err| panic!("{label} INSERT initial rows: {err}"));
        client
            .simple_query("COMMIT")
            .await
            .unwrap_or_else(|err| panic!("{label} commit fixture: {err}"));

        let update_sql = format!("UPDATE \"{schema}\".\"t\" SET v = v + 1 WHERE k IN (1, 2)");

        let messages = client
            .simple_query(&update_sql)
            .await
            .unwrap_or_else(|err| panic!("{label} UPDATE: {err}"));

        let updated_rows = messages
            .iter()
            .find_map(|m| match m {
                SimpleQueryMessage::CommandComplete(n) => Some(*n),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{label} UPDATE should produce CommandComplete"));

        assert_eq!(
            updated_rows, 2,
            "{label}: UPDATE on k IN (1,2) should affect exactly two rows, got {updated_rows}"
        );

        client
            .simple_query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
            .await
            .unwrap_or_else(|err| panic!("{label} DROP SCHEMA: {err}"));

        drop(client);
        let _ = conn_handle.await;
    }
}
