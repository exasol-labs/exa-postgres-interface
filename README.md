# exa-postgres-interface

`exa-postgres-interface` is a PostgreSQL wire-protocol gateway for Exasol. It
lets PostgreSQL-capable tools connect to Exasol through a PostgreSQL-compatible
listener while Exasol remains the actual database engine.

The gateway is a Rust binary built on `pgwire`. It accepts PostgreSQL client
connections, opens one Exasol WebSocket session per client, initializes that
session with an Exasol-side SQL preprocessor, and returns Exasol results using
PostgreSQL protocol messages.

PostgreSQL SQL translation and PostgreSQL catalog compatibility are centered in
Exasol:

* `sql/postgres_catalog_compatibility.sql` creates `PG_CATALOG` and
  `INFORMATION_SCHEMA` compatibility schemas.
* `sql/exasol_sql_preprocessor.sql` installs `PG_DEMO.PG_SQL_PREPROCESSOR`,
  which rewrites PostgreSQL-flavored SQL and metadata probes before Exasol
  executes them.
* The gateway handles protocol behavior, authentication forwarding, session
  setup, TLS policy, capability-based statement routing, and PostgreSQL
  client/session commands.

## Current Status

Implemented:

* PostgreSQL startup and cleartext password authentication.
* Simple Query and Extended Query paths for row-returning statements,
  supported DML, and selected Exasol-equivalent DDL.
* Per-client direct Exasol WebSocket sessions.
* Exasol TLS support with normal certificate validation, SHA-256 fingerprint
  pinning, and a development-only `validate_certificate = false` escape hatch.
* Exasol-side SQL translation with `sqlglot` through
  `PG_DEMO.PG_SQL_PREPROCESSOR`.
* Exasol-side PostgreSQL metadata compatibility schemas:
  `PG_CATALOG` and `INFORMATION_SCHEMA`.
* Full documented PostgreSQL 18 `pg_catalog` and `information_schema` relation
  and column surface, with unsupported objects represented by empty
  PostgreSQL-shaped views.
* Mapped metadata for schemas, tables, views, columns, constraints, indexes,
  roles/users, routines/scripts, and common client catalog helpers where Exasol
  has useful equivalent metadata.
* Compatibility fixes for observed DbVisualizer and DBeaver catalog browser
  queries, including PostgreSQL helper functions and client-specific metadata
  shapes that use arrays, `LATERAL UNNEST`, tuple joins, and PostgreSQL catalog
  aliases.
* JDBC smoke tests, JDBC metadata smoke tests, a broader compatibility suite,
  and a gateway-vs-direct Exasol benchmark harness.
* A systemd unit template for Linux service operation.

Known limits:

* DML is enabled for Exasol-backed execution, and selected DDL is enabled for
  Exasol-equivalent table, view, and schema operations. Broader PostgreSQL
  object families remain capability-gated.
* PostgreSQL transaction wrappers are acknowledged for client compatibility;
  full PostgreSQL transaction semantics, savepoints, and two-phase commit are
  not implemented.
* PostgreSQL SQL cursors are implemented as gateway-managed, materialized,
  read-only cursor state for supported row-returning queries. Binary cursors,
  updatable cursors, positioned writes, and unbounded/spill-managed cursor
  policies are not implemented yet.
* Binary prepared-statement parameters are not implemented.
* PostgreSQL catalog compatibility is broad enough to expose the documented
  surface, but many PostgreSQL-only features intentionally return empty rows or
  `NULL` columns because Exasol has no equivalent object type.
* Compatibility is strongest for JDBC, DbVisualizer, DBeaver, `psql`, and the
  tested metadata/browser paths. Other PostgreSQL tools may still expose
  PostgreSQL-specific metadata or SQL constructs that need additional mapping.

## Deployment Overview

A normal Linux deployment has four parts:

1. Install the Exasol-side compatibility SQL.
2. Install the gateway binary and config on the host.
3. Register the systemd service.
4. Open the PostgreSQL listener port to trusted client IPs.

The examples below assume:

* gateway install root: `/opt/exa-postgres-interface`
* config path: `/etc/exa-postgres-interface/config.toml`
* service user: `exa-postgres-interface`
* PostgreSQL listener: `0.0.0.0:15432`
* Exasol endpoint: `EXASOL_HOST:8563`

## Build A Binary

Until release artifacts are published, build the binary on a compatible Linux
host:

```bash
cargo build --release
```

The gateway binary will be:

```bash
target/release/exa-postgres-interface
```

There is also a development helper binary:

```bash
target/release/exasol_exec
```

`exasol_exec` is useful for installing and probing Exasol SQL directly, but it
is not required by the systemd service.

## Install Exasol-Side SQL

Install the compatibility schemas first:

```bash
python3 scripts/exasol_exec.py \
  --dsn EXASOL_HOST:8563 \
  --user sys \
  --password 'EXASOL_PASSWORD' \
  --file sql/postgres_catalog_compatibility.sql
```

Install the SQL preprocessor:

```bash
python3 scripts/exasol_exec.py \
  --dsn EXASOL_HOST:8563 \
  --user sys \
  --password 'EXASOL_PASSWORD' \
  --file sql/exasol_sql_preprocessor.sql
```

Verify the installed objects:

```sql
SELECT SCRIPT_SCHEMA, SCRIPT_NAME
FROM SYS.EXA_ALL_SCRIPTS
WHERE SCRIPT_SCHEMA = 'PG_DEMO'
  AND SCRIPT_NAME = 'PG_SQL_PREPROCESSOR';

SELECT COUNT(*)
FROM PG_CATALOG.PG_CLASS;

SELECT COUNT(*)
FROM INFORMATION_SCHEMA.TABLES;
```

## Install The Gateway Files

Create the service user and directories:

```bash
sudo useradd --system --home /opt/exa-postgres-interface --shell /usr/sbin/nologin exa-postgres-interface
sudo mkdir -p /opt/exa-postgres-interface/bin
sudo mkdir -p /etc/exa-postgres-interface
```

Copy the release binary:

```bash
sudo install -m 0755 target/release/exa-postgres-interface \
  /opt/exa-postgres-interface/bin/exa-postgres-interface
```

Create the config:

```bash
sudo install -m 0640 -o root -g exa-postgres-interface \
  config/example.toml \
  /etc/exa-postgres-interface/config.toml
```

Edit `/etc/exa-postgres-interface/config.toml`:

```toml
[server]
listen_host = "0.0.0.0"
listen_port = 15432
log_level = "INFO"

# Optional PostgreSQL wire-protocol TLS. Enable this when clients require
# sslmode=require or equivalent.
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
sql_preprocessor_script = "PG_DEMO.PG_SQL_PREPROCESSOR"
session_init_sql = [
  "ALTER SESSION SET SQL_PREPROCESSOR_SCRIPT = {script}"
]
```

The PostgreSQL client username and password are passed through to Exasol when
`pass_client_credentials = true`.

## Install systemd Unit

Copy the unit file:

```bash
sudo install -m 0644 packaging/exa-postgres-interface.service \
  /etc/systemd/system/exa-postgres-interface.service
```

Reload systemd and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now exa-postgres-interface
```

Check status and logs:

```bash
systemctl status exa-postgres-interface
journalctl -u exa-postgres-interface -f
```

## Open The Listener Port

If clients connect from outside the host, the gateway must listen on a reachable
interface:

```toml
[server]
listen_host = "0.0.0.0"
listen_port = 15432
```

Open TCP `15432` only from trusted client IPs. On AWS, add an inbound security
group rule for the client source IP or VPN/CIDR. On a Linux firewall, use the
site's standard firewall tooling to allow the same restricted source.

## Connect A PostgreSQL Client

Use the PostgreSQL driver/connector:

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

## Sample Data

The sample-data helper uses `EXAPUMP_PROFILE=nc-personal-2` by default:

```bash
scripts/setup_sample_data.sh
```

Override the profile when needed:

```bash
EXAPUMP_PROFILE=other-profile scripts/setup_sample_data.sh
```

The sample data creates a richer demo environment with users, roles, nested role
grants, schemas, tables, views, functions, scripts, constraints, and
cross-schema dependencies so metadata browsers have realistic objects to show.

## Test And Compatibility Checks

Rust checks:

```bash
cargo fmt --check
cargo test
cargo build --release
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

See:

* [docs/smoke-test.md](docs/smoke-test.md)
* [docs/postgres-metadata-compatibility.md](docs/postgres-metadata-compatibility.md)
* [docs/client-compatibility-test-framework.md](docs/client-compatibility-test-framework.md)

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

## Release Binary Plan

The repository currently supports building a Linux release binary with Cargo.
The intended distribution shape is:

* publish `exa-postgres-interface` as a Linux release artifact on GitHub;
* include checksums;
* keep `packaging/exa-postgres-interface.service` as the reference systemd
  unit;
* install the binary under `/opt/exa-postgres-interface/bin`;
* keep secrets and deployment-specific config under `/etc/exa-postgres-interface`.

Release automation is not implemented yet. Until then, build from source on a
compatible Linux host and install the resulting `target/release` binary.
