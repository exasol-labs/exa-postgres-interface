# Installation

This page covers everything beyond the one-step installer in the
[Quick Start](../README.md#quick-start): manual tarball installs, the
non-interactive catalog setup, the full configuration reference, and running
the gateway as a systemd service.

> **Network:** the gateway listens on TCP **`15432`** by default. Open that
> port on the host (and any firewall/security group in front of it) so
> PostgreSQL clients can reach the gateway. The gateway in turn connects
> outbound to Exasol on `8563`.

## One-step installer (recap)

```bash
curl -fsSL https://github.com/exasol-labs/exa-postgres-interface/releases/latest/download/install.sh | sh
```

The installer downloads the latest release tarball, verifies its SHA256,
unpacks it, and launches the interactive bootstrap that writes the config and
installs the `PG_CATALOG`/`INFORMATION_SCHEMA` compatibility objects.

It drops the gateway under `/opt/exa-postgres-interface` when run as root, or
`$HOME/.local/exa-postgres-interface` otherwise, and symlinks
`/usr/local/bin/exa-postgres-interface` when running as root. Override the
behavior with environment variables:

| Variable | Effect |
| --- | --- |
| `INSTALL_PREFIX=/path` | Install to a custom directory. |
| `INSTALL_VERSION=v0.2.3` | Pin to a specific release tag instead of latest. |
| `INSTALL_NO_LAUNCH=1` | Install without running the interactive bootstrap. |

## Manual install (release tarball)

If you'd rather drive the steps yourself:

```bash
curl -LO https://github.com/exasol-labs/exa-postgres-interface/releases/download/v0.2.3/exa-postgres-interface-v0.2.3-linux-x86_64.tar.gz
curl -LO https://github.com/exasol-labs/exa-postgres-interface/releases/download/v0.2.3/exa-postgres-interface-v0.2.3-linux-x86_64.tar.gz.sha256
sha256sum -c exa-postgres-interface-v0.2.3-linux-x86_64.tar.gz.sha256
tar -xzf exa-postgres-interface-v0.2.3-linux-x86_64.tar.gz
```

The archive contains the gateway binary, the `exasol_exec` SQL helper,
reference config, the systemd unit, the `PG_CATALOG`/`INFORMATION_SCHEMA`
compatibility SQL, and key docs.

Start the gateway with a config path. If the file does not exist it prompts for
the listener and Exasol connection settings, writes the TOML, and offers to
install the catalog compatibility objects:

```bash
exa-postgres-interface-v0.2.3-linux-x86_64/bin/exa-postgres-interface \
  --config config/local.toml
```

## Non-interactive catalog install

The gateway needs the `PG_CATALOG`/`INFORMATION_SCHEMA` compatibility objects
present in Exasol. The interactive bootstrap installs them for you; to do it
without prompts (e.g. in automation), run:

```bash
exa-postgres-interface-v0.2.3-linux-x86_64/bin/exasol_exec \
  --dsn EXASOL_HOST:8563 \
  --user sys \
  --password 'EXASOL_PASSWORD' \
  --file exa-postgres-interface-v0.2.3-linux-x86_64/sql/postgres_catalog_compatibility.sql
```

## Configuration

Minimal TOML:

```toml
[server]
listen_host = "0.0.0.0"
listen_port = 15432
log_level = "INFO"

[exasol]
dsn = "EXASOL_HOST:8563"
encryption = true
pass_client_credentials = true
certificate_fingerprint = "SHA256_HEX_FINGERPRINT"

[translation]
enabled = true
```

Key settings:

| Setting | Purpose |
| --- | --- |
| `server.listen_host` | `0.0.0.0` to accept remote clients, `127.0.0.1` for same-host only. |
| `server.listen_port` | PostgreSQL listener port (default `15432`). |
| `server.tls_cert_path` / `server.tls_key_path` | Optional PostgreSQL wire-protocol TLS. Set both to accept client `SSLRequest` connections. |
| `exasol.dsn` | Exasol host and port (`HOST:8563`). |
| `exasol.certificate_fingerprint` | Recommended for self-signed Exasol Personal certificates. |
| `exasol.pass_client_credentials` | Authenticate to Exasol with the client's own credentials. |
| `exasol.transport` | `arrow` (default, via [`exarrow-rs`](https://github.com/exasol-labs/exarrow-rs)) or `websocket` for the WebSocket JSON protocol. Fixed at startup. |
| `translation.enabled` | Run PostgreSQL→Exasol dialect translation inside the gateway (default `true`). |

See [config/example.toml](../config/example.toml) for the fully annotated
reference, including the optional legacy database-side SQL preprocessor
fallback.

## Run as a systemd service

```bash
sudo install -m 0755 exa-postgres-interface-v0.2.3-linux-x86_64/bin/exa-postgres-interface /opt/exa-postgres-interface/bin/exa-postgres-interface
sudo install -m 0640 -o root -g exa-postgres-interface exa-postgres-interface-v0.2.3-linux-x86_64/config/example.toml /etc/exa-postgres-interface/config.toml
sudo install -m 0644 exa-postgres-interface-v0.2.3-linux-x86_64/packaging/exa-postgres-interface.service /etc/systemd/system/exa-postgres-interface.service
sudo systemctl daemon-reload && sudo systemctl enable --now exa-postgres-interface
```

The service runs with `--no-bootstrap`; run the interactive bootstrap or the
[non-interactive catalog install](#non-interactive-catalog-install) once before
enabling it.
