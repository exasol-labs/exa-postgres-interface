# Feature: PostgreSQL Session Management

The protocol server SHALL support gateway-managed session behaviors including SQL cursors, prepared statements, and search_path management. These behaviors are implemented at the gateway layer to provide PostgreSQL-compatible semantics over an Exasol session opened through the `exarrow-rs` driver.

## Background

* The client connects using a PostgreSQL-compatible client driver.
* The protocol server opens one Exasol session per accepted client session through the `exarrow-rs` driver.
* The gateway-managed cursor registry holds materialised Apache Arrow `RecordBatch` data, not pre-stringified rows.
* PostgreSQL clients use `SET search_path` and `SHOW search_path` to scope or inspect the active schema; Exasol exposes equivalent state through `OPEN SCHEMA` and `current_schema()`.
* SQL cursors, prepared statements, and search_path management require gateway-side state that is per-session and independent of the Exasol execution path.

## Scenarios

### Scenario: SQL cursor declaration is gateway-managed

* *GIVEN* the client has an active session through the protocol server
* *WHEN* the user sends `DECLARE <name> CURSOR FOR <query>`
* *THEN* the server SHOULD create a gateway-managed cursor when `<query>` is a supported row-returning query
* *AND* the server SHALL store cursor metadata in per-session state
* *AND* the server SHALL reject duplicate cursor names within the same session
* *AND* the server SHALL reject cursor declarations whose query or options require unsupported PostgreSQL semantics


### Scenario: SQL cursor fetch returns result rows

* *GIVEN* a gateway-managed cursor exists for the client session
* *WHEN* the user sends `FETCH` for that cursor
* *THEN* the server SHALL return a PostgreSQL-compatible row description and data rows for the requested cursor slice
* *AND* the server SHALL update the cursor position according to the supported fetch direction
* *AND* the server SHALL return a `FETCH <count>` command completion tag


### Scenario: SQL cursor movement and cleanup are explicit

* *GIVEN* a gateway-managed cursor exists for the client session
* *WHEN* the user sends `MOVE` or `CLOSE`
* *THEN* `MOVE` SHOULD adjust the cursor position without returning rows when the requested movement is supported
* *AND* `CLOSE` SHALL release gateway cursor state
* *AND* the server SHALL release all non-hold cursor state at the documented transaction or session boundary


### Scenario: Unsupported cursor semantics fail safely

* *GIVEN* the client requests cursor behavior such as binary cursors, positioned updates, `WHERE CURRENT OF`, unsupported scroll direction, or unsupported holdability
* *WHEN* the server cannot provide that behavior without changing material semantics
* *THEN* the server SHALL reject the cursor command with a PostgreSQL-compatible error
* *AND* the server SHALL NOT silently downgrade to a different cursor behavior


### Scenario: Prepared statements and protocol portals are designed with write support

* *GIVEN* a client uses PostgreSQL extended query messages or SQL `PREPARE` and `EXECUTE`
* *WHEN* the prepared statement contains supported read or write SQL
* *THEN* the server SHOULD preserve statement classification, parameter typing, translation, execution, and response mapping across parse, bind, execute, and sync phases
* *AND* the server SHALL reject unsupported parameter modes or binary formats with a clear PostgreSQL-compatible error


### Scenario: Single-schema search_path opens the Exasol schema

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `SET search_path = <schema>` or `SET search_path TO <schema>` with exactly one schema identifier
* *THEN* the server SHALL classify the statement as a gateway-managed search_path assignment
* *AND* the server SHALL execute `OPEN SCHEMA <schema>` against Exasol using the active session
* *AND* the server SHALL update gateway-managed session state so that the active schema reflects the new value
* *AND* the server SHALL return a PostgreSQL-compatible `SET` command completion response


### Scenario: Multi-schema search_path is rejected with a compatibility error

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `SET search_path` with more than one comma-separated schema identifier
* *THEN* the server SHALL reject the statement with a PostgreSQL-compatible error stating that only single-schema search paths are supported
* *AND* the server SHALL NOT change gateway-managed session state or issue any `OPEN SCHEMA` against Exasol
* *AND* the server SHALL keep the client session usable for subsequent statements


### Scenario: search_path reset is a no-op

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `RESET search_path`, `SET search_path = DEFAULT`, or `SET search_path TO DEFAULT`
* *THEN* the server SHALL accept the statement and return a PostgreSQL-compatible command completion response
* *AND* the server SHALL NOT issue any Exasol statement on behalf of the request
* *AND* the server SHALL leave gateway-managed session state unchanged because Exasol has no documented "close schema" command


### Scenario: SHOW search_path reflects the active schema

* *GIVEN* the client has an active Exasol session through the protocol server
* *WHEN* the client sends `SHOW search_path`
* *THEN* the server SHALL return the gateway-tracked active schema when one has been set during the session
* *AND* the server SHALL return the documented default value when no schema has been opened during the session
* *AND* the response column name SHALL match the PostgreSQL `SHOW search_path` convention


### Scenario: Cursors materialise Arrow record batches from Exasol

* *GIVEN* the client has an active session through the protocol server
* *AND* the client issues `DECLARE <cursor> FOR <supported row-returning query>`
* *WHEN* the gateway executes the cursor query against Exasol through `exarrow-rs`
* *THEN* the gateway SHALL store the result inside the cursor registry as Apache Arrow `RecordBatch` values
* *AND* the gateway SHALL serve subsequent `FETCH` and `MOVE` commands from the stored `RecordBatch` values without re-executing the query
* *AND* the gateway SHALL encode fetched rows for PostgreSQL using the same Arrow-to-PostgreSQL rendering as ad-hoc queries


### Scenario: search_path open failure surfaces as a PostgreSQL-compatible error

* *GIVEN* the client sends `SET search_path TO "Nonexistent"`
* *WHEN* Exasol rejects the `OPEN SCHEMA` statement executed through `exarrow-rs`
* *THEN* the server SHALL surface the failure as a PostgreSQL-compatible `ErrorResponse`
* *AND* the server SHALL leave the previously active schema unchanged in the session
* *AND* the server SHALL keep the client session usable for later supported statements
