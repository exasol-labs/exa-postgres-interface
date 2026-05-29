// Integration tests for ExasolSession via the exarrow-rs async driver.
// These tests require a live Exasol instance and are skipped by default.
// Run with: cargo test --test exasol_session_integration -- --ignored

mod common;

use arrow::array::{Array, AsArray, Decimal128Array, Int64Array, StringArray};
use arrow::datatypes::DataType;
use exa_postgres_interface::exasol::{ExasolOutcome, ExasolSession};

use common::{TEST_PASSWORD, TEST_USER, all_transport_configs, live_exasol_config_for_transport};

/// Pull the first row, first column of a single-cell result back as a `String`
/// by leaning on Arrow's built-in display utility. Tests use this to assert on
/// the scalar value returned by simple `SELECT` statements without caring about
/// the underlying Arrow type.
fn first_cell_display(outcome: &ExasolOutcome) -> String {
    let batches = match outcome {
        ExasolOutcome::ArrowRows(batches) => batches,
        ExasolOutcome::TypedRows { .. } => panic!("expected ArrowRows outcome, got TypedRows"),
        ExasolOutcome::RowCount(_) => panic!("expected ArrowRows outcome, got RowCount"),
    };
    let batch = batches.first().expect("at least one batch");
    assert!(batch.num_rows() >= 1, "expected at least one row");
    let column = batch.column(0);
    arrow::util::display::array_value_to_string(column, 0).expect("array value renders as string")
}

#[tokio::test]
#[ignore = "live exasol"]
async fn client_credentials_authenticate_through_exarrow() {
    let config = live_exasol_config_for_transport("arrow");
    let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
        .await
        .expect("connect with valid client credentials");

    let outcome = session
        .execute("SELECT CURRENT_USER")
        .await
        .expect("SELECT CURRENT_USER succeeds");

    assert_eq!(first_cell_display(&outcome), "SYS");

    session.close().await.expect("close session");
}

#[tokio::test]
#[ignore = "live exasol"]
async fn empty_arrow_result_preserves_schema() {
    // Empty result sets must still carry the Arrow schema so the gateway can
    // build a PostgreSQL RowDescription. Regression test for the Qlik /
    // pgAdmin NRE on empty getTables() responses.
    let config = live_exasol_config_for_transport("arrow");
    let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
        .await
        .expect("connect");

    let outcome = session
        .execute("SELECT 1 AS A, CAST('x' AS VARCHAR(10)) AS B WHERE 1 = 0")
        .await
        .expect("empty SELECT");

    let batches = match outcome {
        ExasolOutcome::ArrowRows(batches) => batches,
        other => panic!("expected ArrowRows, got {other:?}"),
    };

    let batch = batches
        .first()
        .expect("an empty-row batch must still be present so the schema flows downstream");
    assert_eq!(batch.num_rows(), 0, "no rows are expected");
    assert_eq!(
        batch.num_columns(),
        2,
        "schema columns A and B must survive"
    );
    assert_eq!(batch.schema().field(0).name(), "A");
    assert_eq!(batch.schema().field(1).name(), "B");

    session.close().await.expect("close");
}

#[tokio::test]
#[ignore = "live exasol"]
async fn concurrent_clients_serialize_through_tokio_mutex() {
    // Spin up three independent sessions concurrently; each runs the same
    // metadata query and they MUST all observe the same row count. We hold
    // each session inside an async block so the .await points are forced to
    // interleave on the Tokio runtime — this exercises the async transport
    // path that replaced the old spawn_blocking shim.
    let mut handles = Vec::new();
    for client_id in 0..3 {
        handles.push(tokio::spawn(async move {
            let config = live_exasol_config_for_transport("arrow");
            let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
                .await
                .unwrap_or_else(|err| panic!("client {client_id} connect: {err}"));
            let outcome = session
                .execute("SELECT COUNT(*) FROM SYS.EXA_ALL_USERS")
                .await
                .unwrap_or_else(|err| panic!("client {client_id} query: {err}"));
            let count = match outcome {
                ExasolOutcome::ArrowRows(batches) => extract_count(&batches),
                ExasolOutcome::TypedRows { .. } => {
                    panic!("client {client_id}: expected ArrowRows, got TypedRows")
                }
                ExasolOutcome::RowCount(_) => panic!("client {client_id}: expected ArrowRows"),
            };
            session
                .close()
                .await
                .unwrap_or_else(|err| panic!("client {client_id} close: {err}"));
            count
        }));
    }

    let mut counts = Vec::new();
    for handle in handles {
        counts.push(handle.await.expect("worker task panicked"));
    }

    let first = counts[0];
    assert!(first > 0, "EXA_ALL_USERS should not be empty");
    assert!(
        counts.iter().all(|c| *c == first),
        "all concurrent clients should see the same user count, got {counts:?}"
    );
}

/// Coerce the first cell of a `COUNT(*)`-shaped result to `i64`. Exasol emits
/// `COUNT(*)` as `DECIMAL(18,0)` which exarrow-rs surfaces as a Decimal128
/// Arrow array; some driver versions instead expose it as Int64 or as text,
/// so we accept any of those shapes.
fn extract_count(batches: &[arrow::array::RecordBatch]) -> i64 {
    let batch = batches.first().expect("at least one batch");
    assert_eq!(batch.num_columns(), 1, "COUNT(*) returns one column");
    assert_eq!(batch.num_rows(), 1, "COUNT(*) returns one row");
    let column = batch.column(0);
    match column.data_type() {
        DataType::Int64 => column
            .as_any()
            .downcast_ref::<Int64Array>()
            .expect("Int64Array")
            .value(0),
        DataType::Decimal128(_, _) => {
            let arr = column
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .expect("Decimal128Array");
            // COUNT(*) has scale 0, so the i128 value is the integer count.
            i64::try_from(arr.value(0)).expect("count fits in i64")
        }
        DataType::Utf8 => column
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("StringArray")
            .value(0)
            .parse::<i64>()
            .expect("parse count"),
        other => {
            let rendered = column.as_string::<i32>();
            panic!("unexpected COUNT(*) Arrow type {other:?}, raw={rendered:?}")
        }
    }
}

#[tokio::test]
#[ignore = "live exasol"]
async fn client_credentials_authenticate_under_each_transport() {
    // Drive `ExasolSession::connect` once per configured transport so the
    // adapter contract is exercised end-to-end on both the Arrow and the
    // WebSocket JSON paths. The body asserts that the session can be opened,
    // a `SELECT` returns a row outcome (not a row-count outcome), and the
    // session can be closed cleanly.
    for tcfg in all_transport_configs() {
        let mut session = ExasolSession::connect(&tcfg.exasol_config, TEST_USER, TEST_PASSWORD)
            .await
            .unwrap_or_else(|err| panic!("connect failed for {}: {err}", tcfg.transport));
        let outcome = session
            .execute("SELECT CURRENT_USER")
            .await
            .unwrap_or_else(|err| panic!("execute failed for {}: {err}", tcfg.transport));
        match outcome {
            ExasolOutcome::ArrowRows(_) | ExasolOutcome::TypedRows { .. } => {}
            ExasolOutcome::RowCount(_) => {
                panic!("expected rows, got row count for {}", tcfg.transport)
            }
        }
        session
            .close()
            .await
            .unwrap_or_else(|err| panic!("close failed for {}: {err}", tcfg.transport));
    }
}

#[tokio::test]
#[ignore = "live exasol"]
async fn concurrent_clients_serialize_under_each_transport() {
    // Run three concurrent `SELECT 1` executions per transport, each in its
    // own session, all running simultaneously via `tokio::join!`. The point
    // is to prove that the new transport-trait dispatch holds up under
    // concurrent driver use — neither the Arrow nor the WebSocket transport
    // should deadlock or serialise across independent `ExasolSession`s.
    for tcfg in all_transport_configs() {
        let label = tcfg.transport;
        let mut handles = Vec::new();
        for client_id in 0..3 {
            let config = tcfg.exasol_config.clone();
            handles.push(tokio::spawn(async move {
                let mut session = ExasolSession::connect(&config, TEST_USER, TEST_PASSWORD)
                    .await
                    .unwrap_or_else(|err| panic!("{label} client {client_id} connect: {err}"));
                let outcome = session
                    .execute("SELECT 1")
                    .await
                    .unwrap_or_else(|err| panic!("{label} client {client_id} query: {err}"));
                match outcome {
                    ExasolOutcome::ArrowRows(_) | ExasolOutcome::TypedRows { .. } => {}
                    ExasolOutcome::RowCount(_) => {
                        panic!("{label} client {client_id}: expected rows, got row count")
                    }
                }
                session
                    .close()
                    .await
                    .unwrap_or_else(|err| panic!("{label} client {client_id} close: {err}"));
            }));
        }
        let results = futures::future::join_all(handles).await;
        for (client_id, joined) in results.into_iter().enumerate() {
            joined.unwrap_or_else(|err| panic!("{label} client {client_id} task panicked: {err}"));
        }
    }
}
