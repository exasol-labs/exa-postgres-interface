# Spike: Polyglot Versus SQLGlot Translation Parity

Date: 2026-05-05

## Question

Can the gateway move PostgreSQL-to-Exasol translation from the Exasol-side SQLGlot preprocessor into the Rust gateway using Polyglot without losing feature parity?

## Short Answer

Polyglot is viable as the gateway-owned generic parser/transpiler candidate, but it is not a drop-in replacement for the current `PG_DEMO.PG_SQL_PREPROCESSOR`.

Raw Polyglot and raw SQLGlot are broadly similar on basic PostgreSQL-to-Exasol transpilation, but both fail most observed client metadata queries without the project-owned rewrite layer. The current compatibility behavior comes from SQLGlot plus local edge-case rewrites. A gateway migration MUST port those rewrites into Rust fixtures and run them around Polyglot.

## Sources Checked

* Polyglot Rust/WASM introduction: <https://tobilg.com/posts/introducing-polyglot-a-rust-wasm-sql-transpilation-library/>
* Polyglot Rust API docs: <https://docs.rs/polyglot-sql/latest/polyglot_sql/>
* Polyglot `transpile_by_name` API: <https://docs.rs/polyglot-sql/latest/polyglot_sql/fn.transpile_by_name.html>
* SQLGlot API docs: <https://sqlglot.com/sqlglot.html>
* SQLGlot Exasol dialect docs: <https://sqlglot.com/sqlglot/dialects/exasol.html>

## Method

The spike used a temporary Rust helper with `polyglot-sql = "0.3.5"` and `polyglot_sql::transpile_by_name(sql, "postgres", "exasol")`.

The comparison used three translation modes:

* `sqlglot_raw`: direct `sqlglot.transpile(sql, read="postgres", write="exasol")`.
* `current_adapter`: the current `PG_DEMO.PG_SQL_PREPROCESSOR` Python body loaded locally from `sql/exasol_sql_preprocessor.sql`.
* `polyglot_raw`: direct Polyglot PostgreSQL-to-Exasol transpilation.

A second pass evaluated:

* `polyglot_with_current_rewrites`: current preprocessor normalization, known metadata-query rewrites, catalog/function rewrites, Polyglot generic transpilation, then existing Exasol edge-case post-processing.

Safe read/query fixtures were executed against Exasol at `18.159.248.251`. DML/DDL fixtures were translated but not broadly executed, except for a small temporary-table DML smoke check.

## Fixture Set

Fixtures covered:

* Baseline `SELECT 1`.
* PostgreSQL casts and `ILIKE`.
* PostgreSQL regex operators.
* `OPERATOR(PG_CATALOG.=)` qualified operators.
* DBVisualizer tablespace query using `pg_size_pretty`, `pg_tablespace_size`, and `pg_tablespace_location`.
* Qlik table-list query with unsized `::varchar` casts after translation.
* Qlik column-list query with `type_name, * FROM INFORMATION_SCHEMA.COLUMNS`.
* Metabase foreign-key query with `ANY(c.conkey)` and quoted catalog identifiers.
* Metabase primary-key query with `information_schema._pg_expandarray(i.indkey)`.
* DbVisualizer/JDBC tuple equality against `INFORMATION_SCHEMA.ELEMENT_TYPES`.
* Representative `INSERT`, `UPDATE ... FROM`, and `CREATE TABLE ... SERIAL` statements.

## Results

### Raw Engine Comparison

| Comparison | Both generated SQL | Normalized output identical | Main differences |
| --- | ---: | ---: | --- |
| Raw SQLGlot vs raw Polyglot | 13/13 | 7/13 | Alias formatting, client metadata edge cases, `UPDATE ... FROM` alias rendering |
| Current adapter vs raw Polyglot | 13/13 | 3/13 | Missing catalog/function qualification and metadata rewrites in raw Polyglot |
| Current adapter vs raw SQLGlot | 13/13 | 4/13 | Current adapter adds required local rewrites beyond SQLGlot |

Safe Exasol execution results:

| Mode | Safe read fixtures executed successfully |
| --- | ---: |
| Raw SQLGlot | 2/10 |
| Current adapter | 9/10 |
| Raw Polyglot | 2/10 |

The one current-adapter failure was the synthetic literal-left `ILIKE` fixture. It is not one of the observed client metadata failures, but it should become a separate rewrite/test case if the gateway claims broad `ILIKE` support.

### Polyglot With Current Rewrite Pipeline

| Mode | Safe read fixtures executed successfully |
| --- | ---: |
| Polyglot plus current rewrite pipeline | 9/10 |

The Polyglot pipeline matched current adapter behavior closely. Differences were mostly formatting or optional `AS` alias emission. Observed metadata fixtures executed successfully when current project rewrites were applied around Polyglot.

## Findings

Raw Polyglot is not feature-equivalent to the current preprocessor from a user-visible standpoint. It parses/generates SQL for the fixtures, but it does not know this project's PostgreSQL catalog compatibility strategy.

Raw SQLGlot is also not feature-equivalent to the current preprocessor. The current implementation depends heavily on project-owned rewrites for catalog relations, PostgreSQL helper functions, tuple predicates, client-specific metadata query families, unsized `VARCHAR` casts, and PostgreSQL-only array/catalog constructs.

Polyglot appears viable as the generic transpiler inside a gateway-owned translation pipeline. The spike supports the design direction of moving translation into Rust, but only if the migration includes a first-class rewrite layer and fixtures for every currently supported client edge case.

The highest-risk gap is DML/DDL semantic correctness, not raw parser support. For example, `UPDATE ... FROM (VALUES ...)` generated SQL but Exasol rejected the smoke execution with `UPDATE-target-table must be contained in source tables`. `CREATE TABLE ... SERIAL` also remains a known compatibility risk because both engines emit `SERIAL`, which is PostgreSQL-shaped rather than a proven Exasol identity-column mapping.

## Recommendation

Proceed with a gateway-owned translator spike phase using Polyglot, but do not assume raw Polyglot and SQLGlot are equivalent.

The next implementation plan SHOULD:

1. Introduce a gateway translator interface independent of Polyglot's concrete API.
2. Port current preprocessor behavior into gateway rewrite phases:
   * pre-normalization,
   * known metadata query rewrites,
   * PostgreSQL catalog/function qualification,
   * generic Polyglot transpilation,
   * Exasol edge-case post-processing,
   * unsupported/unsafe translation errors.
3. Convert all current `tests/test_sql_preprocessor.py` cases into gateway translator fixtures.
4. Add live Exasol parse/execution checks for representative DQL, DML, DDL, and metadata SQL.
5. Keep `PG_DEMO.PG_SQL_PREPROCESSOR` as fallback until Polyglot plus gateway rewrites reaches fixture parity and client smoke parity.

Decision: Polyglot should be the preferred gateway-layer candidate, but the migration decision is `Polyglot + project rewrite layer`, not `Polyglot alone`.

## Commands Run

```bash
cargo run --manifest-path /tmp/polyglot_spike/Cargo.toml --quiet
python3 /tmp/compare_translators.py
python3 /tmp/compare_polyglot_pipeline.py
```

