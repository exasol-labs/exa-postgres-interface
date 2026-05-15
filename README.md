# PostgreSQL Gateway for Exasol

A PostgreSQL wire-protocol gateway for Exasol. PostgreSQL-capable tools connect
to the gateway; the gateway translates their SQL and metadata calls to Exasol
and proxies the rest. Exasol remains the database engine.

**Current release:** `v0.1.0` — see
[Releases](https://github.com/nconforti93/exa-postgres-interface/releases).
v0.1.0 translates PostgreSQL `SESSION_USER` to Exasol `CURRENT_USER`, softens
multi-schema `SET search_path` statements so the first schema is silently
opened (matching DBeaver's typical `"DEMO","public"` form), and lowercases
`pg_catalog`/`information_schema` in `PG_CATALOG.PG_NAMESPACE.nspname` so
DBeaver renders system types as `numeric(18)` instead of
`"PG_CATALOG"."NUMERIC"(18)`.

## Install

Download the Linux x86_64 release (built for `x86_64-unknown-linux-musl`, no
glibc/OpenSSL dependency):

```bash
curl -LO https://github.com/nconforti93/exa-postgres-interface/releases/download/v0.1.0/exa-postgres-interface-v0.1.0-linux-x86_64.tar.gz
curl -LO https://github.com/nconforti93/exa-postgres-interface/releases/download/v0.1.0/exa-postgres-interface-v0.1.0-linux-x86_64.tar.gz.sha256
sha256sum -c exa-postgres-interface-v0.1.0-linux-x86_64.tar.gz.sha256
tar -xzf exa-postgres-interface-v0.1.0-linux-x86_64.tar.gz
```

The archive contains the gateway binary, the `exasol_exec` SQL helper,
reference config, the systemd unit, the `PG_CATALOG`/`INFORMATION_SCHEMA`
compatibility SQL, and key docs.

### First run

Start the gateway with a config path. If the file does not exist it prompts for
the listener and Exasol connection settings, writes the TOML, and offers to
install the catalog compatibility objects:

```bash
exa-postgres-interface-v0.1.0-linux-x86_64/bin/exa-postgres-interface \
  --config config/local.toml
```

### Non-interactive catalog install

```bash
exa-postgres-interface-v0.1.0-linux-x86_64/bin/exasol_exec \
  --dsn EXASOL_HOST:8563 \
  --user sys \
  --password 'EXASOL_PASSWORD' \
  --file exa-postgres-interface-v0.1.0-linux-x86_64/sql/postgres_catalog_compatibility.sql
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
sudo install -m 0755 exa-postgres-interface-v0.1.0-linux-x86_64/bin/exa-postgres-interface /opt/exa-postgres-interface/bin/exa-postgres-interface
sudo install -m 0640 -o root -g exa-postgres-interface exa-postgres-interface-v0.1.0-linux-x86_64/config/example.toml /etc/exa-postgres-interface/config.toml
sudo install -m 0644 exa-postgres-interface-v0.1.0-linux-x86_64/packaging/exa-postgres-interface.service /etc/systemd/system/exa-postgres-interface.service
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
cargo test
cargo build --release
```

JDBC compatibility suite against a running gateway:

```bash
scripts/run_jdbc_compatibility_suite.sh \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys 'EXASOL_PASSWORD'
```

Releases are produced by [scripts/package_release.sh](scripts/package_release.sh)
and the GitHub Actions workflow that fires on `v*` tags.

## Performance

Measurable overhead vs. a direct Exasol JDBC connection: roughly `140–155 ms`
fixed cost per small query, `1.11–1.38x` direct-JDBC time on 1M–10M row
transfers, and near-parity once Exasol execution time dominates. Re-benchmark
in your target environment with `scripts/run_gateway_vs_exasol_benchmark.sh`
before treating these numbers as acceptance criteria.
