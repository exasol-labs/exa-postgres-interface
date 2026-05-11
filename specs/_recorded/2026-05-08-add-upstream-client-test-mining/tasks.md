# Tasks: add-upstream-client-test-mining

## Group A (specs and ADR) — COMPLETED
- [x] 1.1 Author ADR `adr-0001-upstream-client-test-mining.md`
- [x] 1.2 Author spec delta `testing/client-compatibility-harness/spec.md`

## Group B (harness plumbing)
- [x] 2.1 Extend QueryProbe with provenance fields and update Reporter
- [x] 2.2 Create `tests/jdbc/upstream-mined/dbeaver/` with README.md
- [x] 2.3 Create `tests/jdbc/upstream-mined/metabase/` with README.md and LICENSE-AGPL.txt

## Group C (mining) [depends on Group B]
- [x] 3.1 Mine ~5 DBeaver SQL probes with provenance [expert]
- [x] 3.2 Mine ~5 Metabase SQL probes with provenance into AGPL-isolated directory [expert]

## Group D (entry point and docs) [depends on Group C]
- [x] 4.1 Update run_jdbc_compatibility_suite.sh to pick up mined sources
- [x] 4.2 Update harness documentation (provenance contract, AGPL rule, baseline-promotion mechanism)

## Group E (verification) [depends on Group D]
- [x] 5.1 Run cargo build, cargo test, cargo fmt --check, and compatibility suite
