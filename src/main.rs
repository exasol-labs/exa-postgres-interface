mod bootstrap;
mod config;
mod exasol;
mod metadata;
mod pg_server;
mod policy;
mod translator;

use std::fs::File;
use std::io::{BufReader, Error as IoError, ErrorKind};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use bootstrap::{BootstrapMode, ensure_config_file, run_interactive_bootstrap};
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::CertificateDer;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tracing::info;

use crate::config::AppConfig;
use crate::pg_server::{ExasolPgWireFactory, ExasolPgWireHandler};

const TOKIO_WORKER_STACK_SIZE: usize = 16 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(TOKIO_WORKER_STACK_SIZE)
        .build()?
        .block_on(run())
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = parse_args()?;
    let config_path = ensure_config_file(args.config_path.clone())?;

    let config = Arc::new(AppConfig::from_file(&config_path)?);
    tracing_subscriber::fmt()
        .with_env_filter(config.log_filter())
        .init();

    if matches!(args.bootstrap_mode, BootstrapMode::Interactive) {
        run_interactive_bootstrap(&config, &config_path)?;
    }

    let listen_addr: SocketAddr = format!(
        "{}:{}",
        config.server.listen_host, config.server.listen_port
    )
    .parse()?;

    let handler = Arc::new(ExasolPgWireHandler::new(config.clone()));
    let factory = Arc::new(ExasolPgWireFactory { handler });
    let listener = TcpListener::bind(listen_addr).await?;
    let tls_acceptor = setup_tls(&config)?;

    info!(
        listen = %listen_addr,
        exasol_dsn = %config.exasol.dsn,
        translation = config.translation.enabled,
        tls = tls_acceptor.is_some(),
        "exa-postgres-interface pgwire server listening"
    );

    loop {
        let (socket, peer) = listener.accept().await?;
        let factory = factory.clone();
        let tls_acceptor = tls_acceptor.clone();
        tokio::spawn(async move {
            if let Err(error) = pgwire::tokio::process_socket(socket, tls_acceptor, factory).await {
                tracing::warn!(%peer, %error, "client connection ended with error");
            }
        });
    }
}

#[derive(Debug)]
struct CliArgs {
    config_path: Option<PathBuf>,
    bootstrap_mode: BootstrapMode,
}

fn parse_args() -> Result<CliArgs, Box<dyn std::error::Error + Send + Sync>> {
    let mut config_path = None;
    let mut bootstrap_mode = BootstrapMode::Interactive;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = Some(PathBuf::from(
                    args.next().ok_or("missing value for --config")?,
                ));
            }
            "--no-bootstrap" => bootstrap_mode = BootstrapMode::Skip,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    Ok(CliArgs {
        config_path,
        bootstrap_mode,
    })
}

fn print_help() {
    eprintln!(
        "Usage: exa-postgres-interface [--config <path>] [--no-bootstrap]\n\n\
         If --config is omitted and no default config exists, the binary prompts for configuration.\n\
         Interactive bootstrap can install PG_CATALOG and INFORMATION_SCHEMA compatibility objects without saving database credentials."
    );
}

fn setup_tls(config: &AppConfig) -> Result<Option<TlsAcceptor>, IoError> {
    if config.server.tls_cert_path.trim().is_empty() {
        return Ok(None);
    }

    let certs = certs(&mut BufReader::new(File::open(
        &config.server.tls_cert_path,
    )?))
    .collect::<Result<Vec<CertificateDer>, IoError>>()?;

    let key = private_key(&mut BufReader::new(File::open(
        &config.server.tls_key_path,
    )?))?
    .ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "TLS key file contains no private key",
        )
    })?;

    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| IoError::new(ErrorKind::InvalidInput, err))?;
    server_config.alpn_protocols = vec![b"postgresql".to_vec()];

    Ok(Some(TlsAcceptor::from(Arc::new(server_config))))
}
