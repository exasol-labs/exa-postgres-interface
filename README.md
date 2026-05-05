# exa-postgres-interface

`exa-postgres-interface` is a PostgreSQL wire-protocol gateway for Exasol. It
lets PostgreSQL-capable tools connect to Exasol through a PostgreSQL-compatible
listener while Exasol remains the database engine.

The goal is compatibility, not PostgreSQL emulation. The gateway exposes the
PostgreSQL protocol and PostgreSQL-shaped metadata where that helps existing
tools work with Exasol. SQL execution, storage, privileges, optimizer behavior,
and durable metadata remain Exasol behavior.

## What Is In This Repository

This repository contains:

* a Rust gateway binary built on `pgwire`;
* a PostgreSQL-to-Exasol SQL translation layer using `polyglot-sql` plus local
  compatibility rewrites;
* Exasol-side `PG_CATALOG` and `INFORMATION_SCHEMA` compatibility objects;
* an interactive first-run installer for generating config and installing the
  catalog compatibility layer;
* a systemd service template;
* smoke, JDBC compatibility, metadata compatibility, and benchmark tooling.

The normal runtime path does not require an Exasol SQL preprocessor. An optional
legacy fallback preprocessor is still available as `PG_CATALOG.PG_SQL_PREPROCESSOR`
for migration scenarios.

## Compatibility Snapshot

The gateway currently targets common PostgreSQL client workflows such as
connectivity checks, catalog browsing, JDBC metadata calls, read queries, core
DML, selected DDL, and basic cursor use. It does not claim full PostgreSQL
server compatibility.

| Area | Current support | Notes |
| --- | --- | --- |
| PostgreSQL wire protocol | Partial | Startup, cleartext password auth, Simple Query, Extended Query, row descriptions, data rows, command tags, and errors are implemented. |
| Authentication | Pass-through | PostgreSQL username/password are used to authenticate to Exasol. Other auth methods are not implemented. |
| TLS | Supported | Exasol TLS supports normal validation, certificate fingerprint pinning, and a development-only no-verify mode. PostgreSQL listener TLS is optional. |
| `SELECT` / DQL | Supported with translation | PostgreSQL-flavored SQL is translated in the gateway before Exasol execution. |
| DML | Supported where Exasol has equivalent behavior | `INSERT`, `UPDATE`, `DELETE`, `MERGE`, and `TRUNCATE` are capability-routed, but PostgreSQL-specific semantics may still be rejected. |
| DDL | Selected support | Table, view, schema, and related Exasol-equivalent DDL are enabled. PostgreSQL-only object families remain unsupported. |
| Transactions | Compatibility wrappers | `BEGIN`, `COMMIT`, and `ROLLBACK` are acknowledged for client compatibility. Full PostgreSQL transaction semantics, savepoints, and two-phase commit are not implemented. |
| Cursors | Gateway-managed | Materialized read-only SQL cursors are supported for row-returning queries. Binary cursors, updatable cursors, and positioned writes are unsupported. |
| Prepared statements | Partial | Extended Query protocol is supported. Binary parameters and SQL-level `PREPARE`/`EXECUTE` behavior are not complete. |
| Metadata | Broad compatibility layer | Exasol-side `PG_CATALOG` and `INFORMATION_SCHEMA` expose PostgreSQL-shaped views/functions backed by Exasol metadata where possible. PostgreSQL-only fields may be empty or `NULL`. |
| Bulk load/export | Not implemented | PostgreSQL `COPY` needs a separate design because Exasol has different import/export semantics. |
| PostgreSQL engine features | Unsupported where no Exasol equivalent exists | Extensions, event triggers, rewrite rules, publications/subscriptions, text search objects, access methods, many tablespace behaviors, and similar PostgreSQL-specific features are not exposed as real Exasol features. |

See the fuller compatibility matrix in
[docs/compatibility-matrix.md](docs/compatibility-matrix.md). Metadata-specific
details are in
[docs/postgres-metadata-compatibility.md](docs/postgres-metadata-compatibility.md).

## How It Works

Each PostgreSQL client connects to the gateway. The gateway opens one Exasol
WebSocket session for that client and passes the client-supplied username and
password through to Exasol.

For SQL sent by clients, the gateway:

1. classifies the statement and rejects unsupported capability families early;
2. handles PostgreSQL client/session commands locally where appropriate;
3. translates supported PostgreSQL-flavored SQL to Exasol SQL in Rust;
4. executes translated SQL through the Exasol WebSocket API;
5. maps Exasol result sets, update counts, and errors back into PostgreSQL
   protocol responses.

PostgreSQL metadata compatibility is split between the gateway and Exasol:

* `sql/postgres_catalog_compatibility.sql` installs `PG_CATALOG` and
  `INFORMATION_SCHEMA` schemas in Exasol.
* The gateway translates PostgreSQL metadata probes and client-specific catalog
  query patterns before execution.
* Exasol compatibility views and functions return Exasol-backed metadata where
  an equivalent exists and stable empty/`NULL` PostgreSQL-shaped results where
  no equivalent exists.

This design keeps installation simpler than the earlier preprocessor approach:
translation ships with the gateway binary, while database-side objects are
limited to stable catalog compatibility.

## Installation

Download the current Linux x86_64 release artifact. The binary is built for
`x86_64-unknown-linux-musl` so it does not depend on the host's glibc or OpenSSL
versions:

```bash
curl -LO https://github.com/nconforti93/exa-postgres-interface/releases/download/v0.0.2/exa-postgres-interface-v0.0.2-linux-x86_64.tar.gz
curl -LO https://github.com/nconforti93/exa-postgres-interface/releases/download/v0.0.2/exa-postgres-interface-v0.0.2-linux-x86_64.tar.gz.sha256
sha256sum -c exa-postgres-interface-v0.0.2-linux-x86_64.tar.gz.sha256
tar -xzf exa-postgres-interface-v0.0.2-linux-x86_64.tar.gz
```

The extracted release directory contains the gateway binary, a small
`exasol_exec` SQL helper, reference config, systemd unit, catalog SQL, optional
fallback preprocessor SQL, and docs:

```bash
exa-postgres-interface-v0.0.2-linux-x86_64/
```

### Interactive First Run

Run the binary from a terminal:

```bash
exa-postgres-interface-v0.0.2-linux-x86_64/bin/exa-postgres-interface --config config/local.toml
```

If the config file does not exist, the gateway prompts for listener and Exasol
connection settings and writes the TOML config.

After loading the config, it prompts for temporary Exasol setup credentials. The
prompt reminds the operator that these credentials are used only to check or
install `PG_CATALOG` and `INFORMATION_SCHEMA`; they are not saved and are not
used for normal SQL processing. The gateway asks before creating or refreshing
compatibility objects.

After the bootstrap check, the gateway starts listening for PostgreSQL clients.

### Non-Interactive Catalog Install

For automation, install the compatibility objects directly:

```bash
exa-postgres-interface-v0.0.2-linux-x86_64/bin/exasol_exec \
  --dsn EXASOL_HOST:8563 \
  --user sys \
  --password 'EXASOL_PASSWORD' \
  --file exa-postgres-interface-v0.0.2-linux-x86_64/sql/postgres_catalog_compatibility.sql
```

Verify the installed objects:

```sql
SELECT COUNT(*) FROM PG_CATALOG.PG_CLASS;
SELECT COUNT(*) FROM INFORMATION_SCHEMA.TABLES;
```

### Configuration

The generated or installed config is TOML:

```toml
[server]
listen_host = "0.0.0.0"
listen_port = 15432
log_level = "INFO"

# Optional PostgreSQL wire-protocol TLS.
# tls_cert_path = "/etc/exa-postgres-interface/server.crt"
# tls_key_path = "/etc/exa-postgres-interface/server.key"

[exasol]
dsn = "EXASOL_HOST:8563"
encryption = true
pass_client_credentials = true
schema = ""

# Prefer certificate pinning for Exasol Personal/self-signed deployments.
certificate_fingerprint = "SHA256_HEX_FINGERPRINT"

# Development only. Do not use this for exposed deployments.
# validate_certificate = false

[translation]
enabled = true
```

See [config/example.toml](config/example.toml) for the reference config.

### systemd

Create the service user and directories:

```bash
sudo useradd --system --home /opt/exa-postgres-interface --shell /usr/sbin/nologin exa-postgres-interface
sudo mkdir -p /opt/exa-postgres-interface/bin
sudo mkdir -p /etc/exa-postgres-interface
```

Install the binary and config:

```bash
sudo install -m 0755 exa-postgres-interface-v0.0.2-linux-x86_64/bin/exa-postgres-interface \
  /opt/exa-postgres-interface/bin/exa-postgres-interface
sudo install -m 0640 -o root -g exa-postgres-interface \
  exa-postgres-interface-v0.0.2-linux-x86_64/config/example.toml \
  /etc/exa-postgres-interface/config.toml
```

Run the interactive bootstrap once before enabling the service, or install the
catalog SQL non-interactively. The service uses `--no-bootstrap` so it never
waits for credentials:

```bash
sudo install -m 0644 exa-postgres-interface-v0.0.2-linux-x86_64/packaging/exa-postgres-interface.service \
  /etc/systemd/system/exa-postgres-interface.service
sudo systemctl daemon-reload
sudo systemctl enable --now exa-postgres-interface
```

Check status and logs:

```bash
systemctl status exa-postgres-interface
journalctl -u exa-postgres-interface -f
```

### Network Access

If clients connect from outside the host, set:

```toml
[server]
listen_host = "0.0.0.0"
listen_port = 15432
```

Open TCP `15432` only from trusted client IPs or network ranges.

### Optional Preprocessor Fallback

The default install does not require an Exasol SQL preprocessor. For a migration
or support scenario that still needs the legacy database-side rewrite path,
install the fallback script:

```bash
exa-postgres-interface-v0.0.2-linux-x86_64/bin/exasol_exec \
  --dsn EXASOL_HOST:8563 \
  --user sys \
  --password 'EXASOL_PASSWORD' \
  --file exa-postgres-interface-v0.0.2-linux-x86_64/sql/exasol_sql_preprocessor.sql
```

Then enable it explicitly:

```toml
[translation]
enabled = true
sql_preprocessor_script = "PG_CATALOG.PG_SQL_PREPROCESSOR"
session_init_sql = [
  "ALTER SESSION SET SQL_PREPROCESSOR_SCRIPT = {script}"
]
```

## Usage

Use a PostgreSQL driver or connector:

* host: gateway host
* port: `15432`
* database: `exasol`
* user: Exasol user
* password: Exasol password

Example with `psql`:

```bash
PGPASSWORD='EXASOL_PASSWORD' psql \
  --host GATEWAY_HOST \
  --port 15432 \
  --username sys \
  --dbname exasol \
  --command 'SELECT 1;'
```

Example JDBC URL:

```text
jdbc:postgresql://GATEWAY_HOST:15432/exasol?preferQueryMode=extended
```

If the client has a separate SSL toggle, either disable client SSL or configure
`server.tls_cert_path` and `server.tls_key_path` so the gateway accepts
PostgreSQL SSLRequest startup packets.

## Development And Testing

Rust checks:

```bash
cargo fmt --check
cargo test
cargo build --release
```

Sample data:

```bash
scripts/setup_sample_data.sh
```

Override the default Exapump profile when needed:

```bash
EXAPUMP_PROFILE=other-profile scripts/setup_sample_data.sh
```

JDBC smoke:

```bash
curl -L -o /tmp/postgresql.jar https://jdbc.postgresql.org/download/postgresql-42.7.8.jar
javac -cp /tmp/postgresql.jar tests/jdbc/PgJdbcSmoke.java
java -cp /tmp/postgresql.jar:tests/jdbc PgJdbcSmoke \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys \
  'EXASOL_PASSWORD'
```

Broader compatibility suite:

```bash
scripts/run_jdbc_compatibility_suite.sh \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys \
  'EXASOL_PASSWORD'
```

Gateway-vs-direct benchmark:

```bash
scripts/run_gateway_vs_exasol_benchmark.sh \
  'jdbc:postgresql://127.0.0.1:15432/exasol?preferQueryMode=extended' \
  sys \
  'EXASOL_PASSWORD' \
  'jdbc:exa:127.0.0.1:8563' \
  sys \
  'EXASOL_PASSWORD'
```

Related docs:

* [docs/compatibility-matrix.md](docs/compatibility-matrix.md)
* [docs/smoke-test.md](docs/smoke-test.md)
* [docs/postgres-metadata-compatibility.md](docs/postgres-metadata-compatibility.md)
* [docs/client-compatibility-test-framework.md](docs/client-compatibility-test-framework.md)

## Release Packaging

Published releases include:

* `exa-postgres-interface-vX.Y.Z-linux-x86_64.tar.gz`
* `exa-postgres-interface-vX.Y.Z-linux-x86_64.tar.gz.sha256`

Release archives are built by [scripts/package_release.sh](scripts/package_release.sh)
and published by the GitHub Actions release workflow when a `v*` tag is pushed.
The archive contains the Linux gateway binary, the `exasol_exec` helper,
reference config, systemd unit, SQL compatibility files, and key docs so end
users do not need Rust or Cargo.

## Performance Notes

This gateway adds measurable overhead compared with a direct Exasol JDBC
connection.

Observed on the current benchmark host:

* tiny result sets usually pay a mostly fixed gateway cost of roughly
  `140-155 ms` per query;
* large result transfers pay an additional payload-dependent cost, with observed
  gateway/direct ratios around `1.11x` to `1.38x` on `1M-10M` row transfers;
* heavier analytical queries returning one row stayed within a few hundred
  milliseconds of direct JDBC because Exasol execution time dominated.

Re-run the benchmark in the target environment before treating these numbers as
acceptance criteria.
