# DBeaver Upstream-Mined SQL Probes

## Contents

This directory contains SQL probe files mined from the DBeaver Community Edition
open-source repository. Each probe exercises a query that real DBeaver sessions
issue against a PostgreSQL-compatible endpoint, surfacing gateway compatibility
gaps that hand-curated probes may miss.

## Upstream Project

**DBeaver Community Edition**
Repository: https://github.com/dbeaver/dbeaver

## Upstream License

Apache License, Version 2.0
https://github.com/dbeaver/dbeaver/blob/devel/LICENSE.md

No separate LICENSE file is needed in this directory because Apache 2.0 is
compatible with the repository's existing license posture.

## Attribution

SQL strings in this directory are derived from DBeaver Community Edition source
code, licensed under the Apache License, Version 2.0. Each probe source file
carries an attribution comment referencing the upstream file path and commit SHA.

## How to Update

Re-mining is manual. To refresh the probes:

1. Check out the DBeaver repository at the pinned commit SHA recorded in each
   probe file's attribution comment.
2. Look in the following upstream path for SQL strings to mine:
   `plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/`
   Key files include `PostgreUtils.java`, `PostgreDialect.java`, and the
   metadata-cache classes under the `model/` subdirectory.
3. Extract SQL literals that represent queries DBeaver issues during connection
   setup, schema introspection, or metadata loading.
4. Add each extracted string as an `EXPLORATORY` probe in this directory with a
   full provenance comment: upstream file path relative to the repository root,
   and the commit SHA at which the string was observed.
5. Update the pinned commit SHA in this README if mining from a newer revision.

## Probes

The following 5 probes are currently in the suite. All were mined from commit
`eb961ed75130078e621fada1f49a4e593d0ce72a` of the DBeaver Community Edition
repository.

| ID | Upstream file | SHA |
|---|---|---|
| `mined-pg-collation` | `plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/model/PostgreDatabase.java` | `eb961ed75130078e621fada1f49a4e593d0ce72a` |
| `mined-pg-tablespace` | `plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/model/PostgreDatabase.java` | `eb961ed75130078e621fada1f49a4e593d0ce72a` |
| `mined-pg-roles` | `plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/model/PostgreDatabase.java` | `eb961ed75130078e621fada1f49a4e593d0ce72a` |
| `mined-pg-language` | `plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/model/PostgreDatabase.java` | `eb961ed75130078e621fada1f49a4e593d0ce72a` |
| `mined-pg-event-trigger` | `plugins/org.jkiss.dbeaver.ext.postgresql/src/org/jkiss/dbeaver/ext/postgresql/model/PostgreEventTrigger.java` | `eb961ed75130078e621fada1f49a4e593d0ce72a` |
