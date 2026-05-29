# Feature: Service Runtime

The prototype SHOULD be installable as a long-running server process on Linux. The preferred operating model is a binary managed by systemd with external configuration and observable logs.

The runtime SHALL connect to Exasol through the `exarrow-rs` async driver. TLS, certificate fingerprint validation, and the `NOCERTCHECK` escape hatch SHALL be configurable on the same surface the gateway has historically exposed.

## Background

* The application runs between PostgreSQL-compatible clients and Exasol.
* The prototype is expected to run on Linux.
* Secrets SHALL NOT be committed to the repository.
* The gateway speaks to Exasol over an encrypted or unencrypted connection chosen by the operator.
* TLS certificate identity SHOULD be verifiable by SHA-256 fingerprint when a public certificate authority is not in use.

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Operator configures Exasol connectivity

* *GIVEN* the operator provides server configuration
* *WHEN* the server starts
* *THEN* the configuration SHALL include the Exasol endpoint needed to create client sessions through the `exarrow-rs` driver
* *AND* the configuration SHOULD allow client-supplied credentials to be passed through to Exasol
* *AND* the configuration SHALL identify any required SQL translation mechanism, including whether translation is gateway-owned or uses an explicitly enabled Exasol-side preprocessor fallback
* *AND* the configuration SHALL accept the existing `encryption`, `certificate_fingerprint`, and `validate_certificate` fields and apply them to the `exarrow-rs` connection parameters

<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: Operator pins the Exasol certificate by SHA-256 fingerprint

* *GIVEN* the operator sets `exasol.certificate_fingerprint` to the SHA-256 hex digest of the Exasol server certificate's DER encoding
* *AND* the operator leaves `exasol.encryption` enabled
* *WHEN* the gateway opens an Exasol session through `exarrow-rs`
* *THEN* the gateway SHALL configure `exarrow-rs` to verify the presented certificate against the configured fingerprint
* *AND* the gateway SHALL refuse the connection with a clear configuration-level error when the presented certificate does not match the fingerprint
* *AND* the gateway SHALL accept fingerprints supplied in upper- or lower-case hexadecimal

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Operator disables Exasol certificate validation

* *GIVEN* the operator sets `exasol.validate_certificate` to `false`
* *AND* the operator does NOT set `exasol.certificate_fingerprint`
* *WHEN* the gateway opens an Exasol session through `exarrow-rs`
* *THEN* the gateway SHALL configure `exarrow-rs` to skip server certificate validation for that session
* *AND* the gateway SHALL log that certificate validation is disabled for the configured Exasol endpoint
* *AND* the gateway MUST NOT silently disable certificate validation when neither escape hatch is configured

<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Operator parses a fingerprint embedded in the Exasol DSN

* *GIVEN* the operator supplies `exasol.dsn = "host/<sha256-hex>:port"` and leaves `exasol.certificate_fingerprint` empty
* *WHEN* the gateway opens an Exasol session through `exarrow-rs`
* *THEN* the gateway SHALL extract the fingerprint segment from the DSN and pass it to the `exarrow-rs` connection parameters
* *AND* the gateway SHALL pass only the host and port (without the fingerprint suffix) as the Exasol endpoint
* *AND* the gateway SHALL prefer `exasol.certificate_fingerprint` over the DSN-embedded fingerprint when both are present

<!-- /DELTA:NEW -->
