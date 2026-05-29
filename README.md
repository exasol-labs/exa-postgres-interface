# PostgreSQL Gateway for Exasol

A PostgreSQL wire-protocol gateway for Exasol. PostgreSQL-capable tools connect
to the gateway; the gateway translates their SQL and metadata calls to Exasol
and proxies the rest. Exasol remains the database engine.

**Current release:** `v0.2.3` — see
[Releases](https://github.com/exasol-labs/exa-postgres-interface/releases).

The gateway talks to Exasol through the
[`exarrow-rs`](https://github.com/exasol-labs/exarrow-rs) Apache Arrow driver
by default. If needed, set `exasol.transport = "websocket"` in the config
file to switch to the WebSocket JSON protocol instead.

## Install

One-step install on Linux x86_64 (downloads the latest release tarball,
verifies the SHA256, unpacks it, and launches the interactive bootstrap that
writes the config and installs the `PG_CATALOG`/`INFORMATION_SCHEMA`
compatibility objects):

```bash
curl -fsSL https://github.com/exasol-labs/exa-postgres-interface/releases/latest/download/install.sh | sh
```

The installer drops the gateway under `/opt/exa-postgres-interface` when run as
root, or `$HOME/.local/exa-postgres-interface` otherwise, and symlinks
`/usr/local/bin/exa-postgres-interface` when running as root. Override the
target with `INSTALL_PREFIX=/path INSTALL_VERSION=v0.2.3 sh` or pin to a
specific release tag with `INSTALL_VERSION=v0.2.3`. Pass
`INSTALL_NO_LAUNCH=1` to install without running the interactive bootstrap.

### Manual install (release tarball)

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

Then start the gateway with a config path. If the file does not exist it
prompts for the listener and Exasol connection settings, writes the TOML, and
offers to install the catalog compatibility objects:

```bash
exa-postgres-interface-v0.2.3-linux-x86_64/bin/exa-postgres-interface \
  --config config/local.toml
```

### Non-interactive catalog install

```bash
exa-postgres-interface-v0.2.3-linux-x86_64/bin/exasol_exec \
  --dsn EXASOL_HOST:8563 \
  --user sys \
  --password 'EXASOL_PASSWORD' \
  --file exa-postgres-interface-v0.2.3-linux-x86_64/sql/postgres_catalog_compatibility.sql
```

### Configuration

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

Optional PostgreSQL listener TLS: set `server.tls_cert_path` and
`server.tls_key_path`. See [config/example.toml](config/example.toml) for the
full reference.

### systemd

```bash
sudo install -m 0755 exa-postgres-interface-v0.2.3-linux-x86_64/bin/exa-postgres-interface /opt/exa-postgres-interface/bin/exa-postgres-interface
sudo install -m 0640 -o root -g exa-postgres-interface exa-postgres-interface-v0.2.3-linux-x86_64/config/example.toml /etc/exa-postgres-interface/config.toml
sudo install -m 0644 exa-postgres-interface-v0.2.3-linux-x86_64/packaging/exa-postgres-interface.service /etc/systemd/system/exa-postgres-interface.service
sudo systemctl daemon-reload && sudo systemctl enable --now exa-postgres-interface
```

The service uses `--no-bootstrap`; run the interactive bootstrap or the
non-interactive catalog install once before enabling.

## Connect

Use any PostgreSQL driver. Host = gateway, port `15432`, database `exasol`,
user/password = Exasol credentials.

```bash
PGPASSWORD='EXASOL_PASSWORD' psql -h GATEWAY_HOST -p 15432 -U sys -d exasol -c 'SELECT 1;'
```

```text
jdbc:postgresql://GATEWAY_HOST:15432/exasol?preferQueryMode=extended
```

## Compatibility

The gateway targets common PostgreSQL client workflows: connectivity, catalog
browsing, JDBC metadata, read queries, core DML, selected DDL, gateway-managed
session commands (`search_path`, `SET application_name`, etc.), and basic
cursors. It does **not** claim full PostgreSQL server compatibility — SQL
execution, storage, privileges, and transaction semantics remain Exasol's.

Unsupported by design: `COPY`, `EXPLAIN`/`ANALYZE`/`VACUUM`, savepoints,
`LOCK TABLE`, extensions, publications, text-search objects, and other
PostgreSQL engine-specific features that lack an Exasol equivalent.

Full matrix and details:

* [docs/compatibility-matrix.md](docs/compatibility-matrix.md)
* [docs/postgres-metadata-compatibility.md](docs/postgres-metadata-compatibility.md)
* [docs/client-compatibility-test-framework.md](docs/client-compatibility-test-framework.md)
* [docs/smoke-test.md](docs/smoke-test.md)

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

Live Exasol integration tests live under `tests/` and are marked `#[ignore]`
so they only run on demand. They expect the test instance defined in
`tests/common/mod.rs` (default `127.0.0.1:9564`, user `sys`). The instance
must already have `PG_CATALOG` and `INFORMATION_SCHEMA` installed — bootstrap
once with:

```bash
python3 scripts/exasol_exec.py \
  --dsn 127.0.0.1:9564 --user sys --password EXASOL_PASSWORD \
  --file sql/postgres_catalog_compatibility.sql
```

Then run the suite:

```bash
cargo test --all -- --ignored --test-threads=1
```

JDBC compatibility suite against a running gateway:

```bash
scripts/run_jdbc_compatibility_suite.sh \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys 'EXASOL_PASSWORD'
```

Releases are produced by [scripts/package_release.sh](scripts/package_release.sh)
and the GitHub Actions workflow that fires on `v*` tags. The script defaults
to `x86_64-unknown-linux-musl`; pass `TARGET_TRIPLE=x86_64-unknown-linux-gnu`
on hosts without the musl cross-toolchain.

## Performance

Measurable overhead vs. a direct Exasol JDBC connection: roughly `140–155 ms`
fixed cost per small query, `1.11–1.38x` direct-JDBC time on 1M–10M row
transfers, and near-parity once Exasol execution time dominates. Re-benchmark
in your target environment with `scripts/run_gateway_vs_exasol_benchmark.sh`
before treating any of these numbers as acceptance criteria.
