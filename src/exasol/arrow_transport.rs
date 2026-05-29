use std::sync::Arc;

use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatch;
use async_trait::async_trait;
use exarrow_rs::Connection;

use crate::config::ExasolConfig;
use crate::exasol::{
    EndpointConnection, ExasolError, ExasolOutcome, ExasolTransport, map_connection_error,
};

/// Arrow transport: thin async wrapper over `exarrow_rs::Connection`.
pub(crate) struct ArrowTransport {
    conn: Connection,
}

impl ArrowTransport {
    pub(crate) async fn connect(
        config: &ExasolConfig,
        endpoint: &EndpointConnection,
        username: &str,
        password: &str,
    ) -> Result<Self, ExasolError> {
        let params = endpoint.to_connection_params(config, username, password)?;
        let conn = Connection::from_params(params)
            .await
            .map_err(map_connection_error)?;
        Ok(Self { conn })
    }
}

#[async_trait]
impl ExasolTransport for ArrowTransport {
    async fn execute(&mut self, sql: &str) -> Result<ExasolOutcome, ExasolError> {
        let result_set = self
            .conn
            .execute(sql)
            .await
            .map_err(|err| ExasolError::Execution(err.to_string()))?;

        if let Some(count) = result_set.row_count() {
            return Ok(ExasolOutcome::RowCount(count));
        }

        // Capture the schema before fetch_all consumes the ResultSet. Exasol
        // queries that return 0 rows hand back an empty Vec<RecordBatch>;
        // without the schema, downstream code in pg_server::cursor_schema_for
        // falls back to Schema::empty(), which sends an empty PostgreSQL
        // RowDescription. Strict JDBC/ADO.NET clients (Qlik, pgAdmin) then
        // null-reference because they expect column metadata on the result
        // set even when row count is zero.
        let schema = result_set
            .metadata()
            .map(|metadata| metadata.schema.clone());

        let batches = result_set
            .fetch_all()
            .await
            .map_err(|err| ExasolError::Execution(err.to_string()))?;

        Ok(ExasolOutcome::ArrowRows(backfill_schema_when_empty(
            batches, schema,
        )))
    }

    async fn execute_update(&mut self, sql: &str) -> Result<(), ExasolError> {
        self.conn
            .execute_update(sql)
            .await
            .map_err(|err| ExasolError::Execution(err.to_string()))?;
        Ok(())
    }

    async fn close(self: Box<Self>) -> Result<(), ExasolError> {
        self.conn
            .close()
            .await
            .map_err(|err| ExasolError::Connection(err.to_string()))
    }
}

/// Backfill an empty `RecordBatch` carrying the captured schema when the
/// result set returned zero rows, so the gateway always has at least one
/// batch from which to derive the PostgreSQL row description.
fn backfill_schema_when_empty(
    batches: Vec<RecordBatch>,
    schema: Option<Arc<Schema>>,
) -> Vec<RecordBatch> {
    if !batches.is_empty() {
        return batches;
    }
    match schema {
        Some(schema) => vec![RecordBatch::new_empty(schema)],
        None => batches,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::Int32Array;
    use arrow::datatypes::{DataType, Field};

    fn schema_a_b() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("A", DataType::Int32, true),
            Field::new("B", DataType::Utf8, true),
        ]))
    }

    #[test]
    fn empty_batches_get_a_schema_carrier_batch() {
        let schema = schema_a_b();
        let result = backfill_schema_when_empty(Vec::new(), Some(schema.clone()));
        assert_eq!(result.len(), 1, "exactly one empty batch is added");
        assert_eq!(result[0].num_rows(), 0);
        assert_eq!(result[0].num_columns(), 2);
        assert_eq!(result[0].schema(), schema);
    }

    #[test]
    fn empty_batches_without_schema_are_left_alone() {
        // If exarrow gives us neither rows nor a schema (shouldn't happen in
        // practice for result-set responses) we must not pretend.
        let result = backfill_schema_when_empty(Vec::new(), None);
        assert!(result.is_empty());
    }

    #[test]
    fn non_empty_batches_are_returned_as_is() {
        let schema = schema_a_b();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3])),
                Arc::new(arrow::array::StringArray::from(vec!["x", "y", "z"])),
            ],
        )
        .expect("build batch");
        let result = backfill_schema_when_empty(vec![batch.clone()], Some(schema));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].num_rows(), 3);
    }
}
