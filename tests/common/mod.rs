//! Shared helpers for live-Exasol integration tests.
//!
//! Every test in this directory that talks to the live instance pulls
//! credentials and bootstrap logic from here so that connection details
//! live in exactly one place.
//!
//! The helpers are deliberately allow-dead-code: not every test file uses
//! every constant or helper, but Cargo compiles `tests/common/mod.rs` once
//! per test binary that pulls it in.

#![allow(dead_code, unused_imports)]

pub mod transport_matrix;

use std::net::SocketAddr;
use std::sync::Arc;

use exa_postgres_interface::config::{
    AppConfig, DEFAULT_TRANSPORT, ExasolConfig, ServerConfig, TranslationConfig,
};
use exa_postgres_interface::pg_server::{ExasolPgWireFactory, ExasolPgWireHandler};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub use transport_matrix::{TransportTestConfig, all_transport_configs};

pub const LIVE_EXASOL_HOST: &str = "127.0.0.1";
pub const LIVE_EXASOL_PORT: u16 = 9564;
pub const TEST_USER: &str = "sys";
pub const TEST_PASSWORD: &str = "exasol";

/// SHA-256 fingerprint of the live Exasol server certificate, captured via
/// `openssl s_client -connect 3.124.151.144:8563 -showcerts | openssl x509 -fingerprint -sha256`
/// during test development. Exasol's `NOCERTCHECK` syntax accepts the hex
/// digest without colons.
pub const LIVE_FINGERPRINT_HEX: &str =
    "A996DAAA5D6AB45075CDC12E8EE219DEE571F8A60FA0E4796C003AC939759393";

/// Build the canonical `ExasolConfig` for the live test instance. The
/// instance presents a self-signed certificate, so validation is disabled
/// by default (callers that want fingerprint pinning can clone and mutate).
pub fn live_exasol_config() -> ExasolConfig {
    ExasolConfig {
        dsn: format!("{LIVE_EXASOL_HOST}:{LIVE_EXASOL_PORT}"),
        encryption: true,
        certificate_fingerprint: String::new(),
        validate_certificate: false,
        pass_client_credentials: true,
        schema: String::new(),
        transport: DEFAULT_TRANSPORT.to_owned(),
    }
}

/// Build the canonical `ExasolConfig` with `transport` explicitly set. Useful
/// for tests that exercise a specific transport rather than the parameterised
/// matrix.
pub fn live_exasol_config_for_transport(transport: &str) -> ExasolConfig {
    ExasolConfig {
        transport: transport.to_owned(),
        ..live_exasol_config()
    }
}

/// Build a `ServerConfig` that listens on `127.0.0.1:<listen_port>` (`0`
/// to let the OS pick a free port). TLS for the pgwire side is disabled.
pub fn live_server_config(listen_port: u16) -> ServerConfig {
    ServerConfig {
        listen_host: "127.0.0.1".to_owned(),
        listen_port,
        log_level: "info".to_owned(),
        tls_cert_path: String::new(),
        tls_key_path: String::new(),
    }
}

/// Build the full `AppConfig` used by the gateway under test. Translation
/// stays disabled by default so that tests can reach Exasol with the
/// minimum amount of session setup.
pub fn live_app_config(listen_port: u16) -> AppConfig {
    AppConfig {
        server: live_server_config(listen_port),
        exasol: live_exasol_config(),
        translation: TranslationConfig {
            enabled: false,
            sql_preprocessor_script: String::new(),
            session_init_sql: Vec::new(),
        },
    }
}

/// Spawn the gateway on an OS-assigned port and return the bound socket
/// address along with the task handle. The accept loop runs until the
/// handle is dropped or the test process exits.
///
/// Connect from a test with
/// `tokio_postgres::connect("host=127.0.0.1 port=<port> user=sys password=exasol", NoTls)`.
pub async fn spawn_gateway() -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral test port");
    let addr = listener.local_addr().expect("listener local_addr");

    let config = Arc::new(live_app_config(addr.port()));
    spawn_gateway_on_listener(listener, config).await
}

/// Spawn the gateway with a caller-supplied `AppConfig`. The config's
/// `server.listen_port` is overwritten with the OS-assigned port so the same
/// `AppConfig` template can be reused across tests without pre-binding.
pub async fn spawn_gateway_with_config(config: Arc<AppConfig>) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral test port");
    let addr = listener.local_addr().expect("listener local_addr");

    let mut config = (*config).clone();
    config.server.listen_port = addr.port();
    spawn_gateway_on_listener(listener, Arc::new(config)).await
}

async fn spawn_gateway_on_listener(
    listener: TcpListener,
    config: Arc<AppConfig>,
) -> (SocketAddr, JoinHandle<()>) {
    let addr = listener.local_addr().expect("listener local_addr");
    let handler = Arc::new(ExasolPgWireHandler::new(config));
    let factory = Arc::new(ExasolPgWireFactory { handler });

    let handle = tokio::spawn(async move {
        loop {
            let (socket, peer) = match listener.accept().await {
                Ok(pair) => pair,
                Err(error) => {
                    eprintln!("test gateway accept failed: {error}");
                    return;
                }
            };
            let factory = factory.clone();
            tokio::spawn(async move {
                if let Err(error) = pgwire::tokio::process_socket(socket, None, factory).await {
                    eprintln!("test gateway client {peer} ended: {error}");
                }
            });
        }
    });

    (addr, handle)
}

/// Build a `tokio_postgres` connection string targeting the local gateway.
pub fn pg_connection_string(addr: SocketAddr) -> String {
    format!(
        "host={host} port={port} user={user} password={pwd} dbname=exasol",
        host = addr.ip(),
        port = addr.port(),
        user = TEST_USER,
        pwd = TEST_PASSWORD,
    )
}
