# Metabase Upstream-Mined SQL Probes

## Contents

This directory contains SQL probe files mined from the Metabase open-source
repository. Each probe exercises a query that real Metabase sessions issue
against a PostgreSQL-compatible endpoint, surfacing gateway compatibility gaps
that hand-curated probes may miss.

## Upstream Project

**Metabase**
Repository: https://github.com/metabase/metabase

## Upstream License

GNU Affero General Public License, Version 3.0 (AGPL-3.0)
https://github.com/metabase/metabase/blob/master/LICENSE-AGPL.txt

## LICENSE WARNING — READ BEFORE USING

**The contents of this directory inherit the AGPL-3.0 license of the upstream
Metabase source code. The gateway runtime artifact (produced by
`cargo build --release`) MUST NOT bundle any file from this directory. This
directory exists only for test-time use.**

The AGPL-3.0 license places network-service distribution obligations on any
software that incorporates or links against AGPL-3.0-licensed material. To avoid
those obligations, the files in this directory must never be included in, or
compiled into, any release artifact.

See `LICENSE-AGPL.txt` in this directory for the full statement.

## Attribution

SQL strings in this directory are derived from Metabase source code, licensed
under the GNU Affero General Public License v3.0 (AGPL-3.0). See
LICENSE-AGPL.txt in this directory.

## How to Update

Re-mining is manual. To refresh the probes:

1. Check out the Metabase repository at the pinned commit SHA recorded in each
   probe file's attribution comment.
2. Look in the following upstream paths for SQL strings to mine:
   - `src/metabase/driver/postgres.clj`
   - The `metabase/driver/sql_jdbc/sync` namespace
3. Extract SQL literals that represent queries Metabase issues during connection
   setup, schema introspection, or query execution.
4. Add each extracted string as an `EXPLORATORY` probe in this directory with a
   full provenance comment: upstream file path relative to the repository root,
   and the commit SHA at which the string was observed. Include a pointer to the
   AGPL notice in this directory.
5. Update the pinned commit SHA in this README if mining from a newer revision.
6. Verify that the new probes are excluded from release artifact packaging before
   merging.

## Probes

The following 5 probes are currently in the suite. All were mined from commit
`8c932afa0b1d37cdfa6994a9fe32f57e74d93fa2` of the Metabase repository.

Two probes (`mined-get-tables`, `mined-describe-fks`) are rendered from
HoneySQL DSL — they are not verbatim string literals from the upstream source,
but the SQL they represent was derived from the upstream HoneySQL expression at
the referenced location.

| ID | Upstream file | SHA |
|---|---|---|
| `mined-pg-enum-types` | `src/metabase/driver/postgres.clj#L174` | `8c932afa0b1d37cdfa6994a9fe32f57e74d93fa2` |
| `mined-show-timezone` | `src/metabase/driver/postgres.clj#L120` | `8c932afa0b1d37cdfa6994a9fe32f57e74d93fa2` |
| `mined-quote-ident` | `src/metabase/driver/postgres.clj#L1182` | `8c932afa0b1d37cdfa6994a9fe32f57e74d93fa2` |
| `mined-get-tables` | `src/metabase/driver/postgres.clj#L160` (HoneySQL DSL rendered) | `8c932afa0b1d37cdfa6994a9fe32f57e74d93fa2` |
| `mined-describe-fks` | `src/metabase/driver/postgres.clj#L279` (HoneySQL DSL rendered) | `8c932afa0b1d37cdfa6994a9fe32f57e74d93fa2` |
