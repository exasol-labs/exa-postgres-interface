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

        let batches = result_set
            .fetch_all()
            .await
            .map_err(|err| ExasolError::Execution(err.to_string()))?;

        Ok(ExasolOutcome::ArrowRows(batches))
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
