# Feature: Service Runtime

The prototype SHOULD be installable as a long-running server process on Linux. The preferred operating model is a binary managed by systemd with external configuration and observable logs.

The runtime SHALL connect to Exasol through a configurable transport. The operator SHALL choose between the `exarrow-rs` Apache Arrow driver (default) and the officially supported Exasol WebSocket JSON API. TLS, certificate fingerprint validation, and the `NOCERTCHECK` escape hatch SHALL be configurable on the same surface the gateway has historically exposed, regardless of the selected transport.

## Background

* The application runs between PostgreSQL-compatible clients and Exasol.
* The prototype is expected to run on Linux.
* Secrets SHALL NOT be committed to the repository.
* The gateway speaks to Exasol over an encrypted or unencrypted connection chosen by the operator.
* TLS certificate identity SHOULD be verifiable by SHA-256 fingerprint when a public certificate authority is not in use.
* The gateway exposes two Exasol transports: the WebSocket JSON transport (`websocket`) and the Apache Arrow transport (`arrow`).

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Operator configures Exasol connectivity

* *GIVEN* the operator provides server configuration
* *WHEN* the server starts
* *THEN* the configuration SHALL include the Exasol endpoint needed to create client sessions
* *AND* the configuration SHOULD allow client-supplied credentials to be passed through to Exasol
* *AND* the configuration SHALL identify any required SQL translation mechanism, including whether translation is gateway-owned or uses an explicitly enabled Exasol-side preprocessor fallback
* *AND* the configuration SHALL accept the existing `encryption`, `certificate_fingerprint`, and `validate_certificate` fields and apply them to whichever transport is selected
* *AND* the configuration SHALL accept an `exasol.transport` field whose value MUST be either `"websocket"` or `"arrow"`
* *AND* the configuration SHALL fall back to the value of the `DEFAULT_TRANSPORT` constant declared in `src/config.rs` when `exasol.transport` is omitted

<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: Operator selects the WebSocket transport

* *GIVEN* the operator sets `exasol.transport = "websocket"`
* *WHEN* the server starts and accepts a PostgreSQL client connection
* *THEN* the gateway SHALL open the Exasol session using the WebSocket JSON transport
* *AND* the gateway SHALL log the selected transport once at startup
* *AND* the gateway SHALL NOT initialise or import the Apache Arrow transport for that session

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Operator selects the Arrow transport

* *GIVEN* the operator sets `exasol.transport = "arrow"` (or omits the field while `DEFAULT_TRANSPORT` equals `"arrow"`)
* *WHEN* the server starts and accepts a PostgreSQL client connection
* *THEN* the gateway SHALL open the Exasol session using the `exarrow-rs` Apache Arrow transport
* *AND* the gateway SHALL log the selected transport once at startup
* *AND* the gateway SHALL NOT open a WebSocket JSON session for that connection

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Operator supplies an unknown transport value

* *GIVEN* the operator sets `exasol.transport` to a value other than `"websocket"` or `"arrow"`
* *WHEN* the server attempts to load configuration
* *THEN* the server SHALL fail startup with a configuration error that names the offending value and lists the accepted values
* *AND* the server SHALL NOT fall back silently to the default transport
* *AND* the server SHALL NOT open any Exasol session

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Switching transports requires a restart

* *GIVEN* the server is running with one transport selected in configuration
* *WHEN* the operator wishes to change the active transport
* *THEN* the operator MUST edit `exasol.transport` in the configuration file and restart the server
* *AND* the server SHALL NOT switch transports for an established session
* *AND* the server SHALL NOT automatically fall back from one transport to the other if a session fails at runtime

<!-- /DELTA:NEW -->

<!-- DELTA:CHANGED -->
### Scenario: Operator pins the Exasol certificate by SHA-256 fingerprint

* *GIVEN* the operator sets `exasol.certificate_fingerprint` to the SHA-256 hex digest of the Exasol server certificate's DER encoding
* *AND* the operator leaves `exasol.encryption` enabled
* *WHEN* the gateway opens an Exasol session through the configured transport
* *THEN* the gateway SHALL configure the active transport to verify the presented certificate against the configured fingerprint
* *AND* the gateway SHALL refuse the connection with a clear configuration-level error when the presented certificate does not match the fingerprint
* *AND* the gateway SHALL accept fingerprints supplied in upper- or lower-case hexadecimal
* *AND* the gateway SHALL apply the same fingerprint policy whether the active transport is `websocket` or `arrow`

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Operator disables Exasol certificate validation

* *GIVEN* the operator sets `exasol.validate_certificate` to `false`
* *AND* the operator does NOT set `exasol.certificate_fingerprint`
* *WHEN* the gateway opens an Exasol session through the configured transport
* *THEN* the gateway SHALL configure the active transport to skip server certificate validation for that session
* *AND* the gateway SHALL log that certificate validation is disabled for the configured Exasol endpoint
* *AND* the gateway MUST NOT silently disable certificate validation when neither escape hatch is configured
* *AND* the gateway SHALL apply the same escape-hatch policy whether the active transport is `websocket` or `arrow`

<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Operator parses a fingerprint embedded in the Exasol DSN

* *GIVEN* the operator supplies `exasol.dsn = "host/<sha256-hex>:port"` and leaves `exasol.certificate_fingerprint` empty
* *WHEN* the gateway opens an Exasol session through the configured transport
* *THEN* the gateway SHALL extract the fingerprint segment from the DSN and pass it to the active transport's connection parameters
* *AND* the gateway SHALL pass only the host and port (without the fingerprint suffix) as the Exasol endpoint
* *AND* the gateway SHALL prefer `exasol.certificate_fingerprint` over the DSN-embedded fingerprint when both are present
* *AND* the gateway SHALL apply the same DSN precedence whether the active transport is `websocket` or `arrow`

<!-- /DELTA:CHANGED -->
