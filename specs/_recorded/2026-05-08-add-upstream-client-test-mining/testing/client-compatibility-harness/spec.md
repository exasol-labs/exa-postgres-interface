# Feature: Client Compatibility Harness

Status as of 2026-05-08: existing harness is being extended with provenance, license-aware corpora, and per-tool baseline promotion. This delta adds investigation-level scenarios produced by the `add-upstream-client-test-mining` plan; full corpora and the baseline-promotion machinery are deferred to follow-up plans.

The repository SHALL provide a repeatable compatibility harness for the PostgreSQL gateway. The harness SHALL report which JDBC metadata calls and PostgreSQL-flavored statement families succeed, fail, or degrade for realistic client personas without assuming full PostgreSQL compatibility. The repository SHALL also provide a repeatable latency benchmark that compares gateway query execution against direct Exasol JDBC for logically equivalent read-heavy queries.

## Background

* The gateway began as a read-mostly PostgreSQL compatibility layer in front of Exasol.
* The first implementation already has narrow smoke coverage for pgJDBC and DbVisualizer.
* Real PostgreSQL clients rely on both JDBC metadata APIs and direct `pg_catalog` or `information_schema` queries.
* The team wants to know which PostgreSQL statement families do not work yet, not only which smoke queries already pass.
* The team also wants to understand gateway overhead for representative read-heavy queries compared with direct Exasol JDBC.
* DBeaver Community Edition is distributed under Apache 2.0 and Metabase under AGPL-3.0; license posture differs per upstream source.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Mined probes carry upstream provenance metadata

* *GIVEN* the compatibility suite includes probes derived from open-source client source code
* *WHEN* the suite registers a mined probe in its corpus
* *THEN* the probe SHALL carry a provenance record naming the upstream project, source file path, upstream commit SHA or tag, and upstream license identifier
* *AND* the provenance record SHALL be visible in the suite report for every mined probe outcome
* *AND* the documentation SHALL state that hand-curated probes without an upstream source MAY omit the provenance fields
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Mined corpora are stored according to upstream license

* *GIVEN* the team mines SQL strings from upstream client source code
* *WHEN* the team adds those strings to the repository
* *THEN* the documentation SHALL record the upstream license for every source project
* *AND* probes mined from Apache 2.0 sources MAY live alongside the existing Apache-compatible test code with attribution comments
* *AND* probes mined from AGPL-3.0 sources SHALL be isolated in a directory whose `README` or `LICENSE` notice declares AGPL-3.0 inheritance for that directory only
* *AND* the build tooling MUST NOT bundle AGPL-derived probe text into the gateway runtime artifact
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Investigation produces an ADR comparing mining and synthetic-pgJDBC approaches

* *GIVEN* the team is evaluating how to expand client compatibility coverage
* *WHEN* the investigation phase concludes
* *THEN* the repository SHALL contain an Architecture Decision Record that compares upstream-source mining against synthetic-pgJDBC protocol coverage
* *AND* the ADR SHALL document the AGPL handling decision for Metabase-derived corpora
* *AND* the ADR SHALL enumerate concrete pgJDBC behavioral surfaces (extended query protocol, parameter-status messages, prepared-statement describe round-trips, server-side cursors, error class-codes) that synthetic coverage SHOULD address in follow-up plans
* *AND* the ADR SHALL identify the follow-up plans required to deliver full corpora and baseline-promotion machinery
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Proof-of-concept demonstrates mining is tractable

* *GIVEN* the investigation includes a small proof-of-concept
* *WHEN* the team adds mined probes to the existing harness
* *THEN* the corpus SHALL include at least five probes mined from DBeaver upstream source with full provenance
* *AND* the corpus SHALL include at least five probes mined from Metabase upstream source with full provenance
* *AND* the proof-of-concept probes SHALL run under the existing single-command compatibility-suite entry point without manual extra steps
* *AND* failures of proof-of-concept probes SHALL be reported as exploratory and MUST NOT cause MUST-PASS regression
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Per-tool baseline promotion mechanism is specified

* *GIVEN* an exploratory probe for a specific client tool has passed at least once in CI
* *WHEN* the team promotes that probe to a regression baseline for that tool
* *THEN* the harness SHALL define an observable mechanism for marking a probe as a regression-baseline for a specific tool, separate from the current global MUST-PASS set
* *AND* the harness SHALL distinguish promoted-probe failures from MUST-PASS failures and from exploratory failures in the final report
* *AND* the implementation of the promotion mechanism MAY be deferred to a follow-up plan, but the specification SHALL be in place before that plan begins
<!-- /DELTA:NEW -->
