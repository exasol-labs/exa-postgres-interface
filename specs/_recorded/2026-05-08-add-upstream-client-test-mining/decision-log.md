# Decision Log: add-upstream-client-test-mining

Date: 2026-05-08

## Interview

**Q:** What should the primary source of new test cases be (mining upstream source, synthetic from pgJDBC, live wire-traffic capture, running upstream test suites)?
**A:** Both "Mine SQL from upstream source" AND "Synthetic from pgJDBC + driver protocols". Live wire-traffic capture and running upstream test suites directly are explicitly excluded.

**Q:** How should DBeaver/Metabase coverage gate CI?
**A:** Per-tool baseline. Once a probe passes in CI it is pinned as a regression for that tool. Newly mined probes stay exploratory until they have passed and are explicitly promoted.

**Q:** What is the priority target client?
**A:** Both DBeaver and Metabase, equal weight. Both are first-class in the plan.

**Q:** Should this be one plan or split across multiple?
**A:** Investigation plan first, implementation later. This plan delivers research plus a small POC. Follow-up plans deliver full per-tool corpora and the baseline-promotion machinery.

**Q:** What should the investigation plan's deliverable look like?
**A:** ADR plus a small proof-of-concept (~5-10 mined probes from each tool wired into the existing harness) demonstrating the mining approach is tractable.

**Q:** How important is automated re-mining as upstream tools update?
**A:** Out of scope for this plan. The freshness/automation question is deferred. This plan only evaluates whether mining is viable at all.

## Design Decisions

### [1] Adopt mining and synthetic pgJDBC as complementary approaches

- **Decision:** Both upstream-source mining (Option 1) and synthetic pgJDBC protocol coverage (Option 2) are adopted. Mining is primary near-term; synthetic is a follow-up plan.
- **Alternatives:** Pick mining only; pick synthetic only; capture-replay; run upstream test suites.
- **Rationale:** Mining catches tool-specific SQL the user is actually seeing fail; synthetic catches protocol-level surfaces the tools' source does not expose as text (extended-query round-trips, parameter-status, cursors, SQLSTATE). Capture-replay and running upstream suites were rejected by the user.
- **Promotes to ADR:** yes

### [2] Isolate Metabase-derived probes under an AGPL-only directory

- **Decision:** Metabase-derived probes live under `tests/jdbc/upstream-mined/metabase/` with a `LICENSE-AGPL.txt` and `README.md` declaring AGPL-3.0 inheritance for that directory only. The gateway runtime artifact MUST NOT bundle that directory.
- **Alternatives:** (a) treat short SQL fragments as non-copyrightable facts under scenes-à-faire; (b) abandon Metabase mining entirely; (c) co-locate with the rest of the test code.
- **Rationale:** Conservative, license-clean, removes ambiguity from the build. Reversible if a future legal review concludes the SQL strings are non-copyrightable facts. Reversing isolation later is cheaper than retrofitting it.
- **Promotes to ADR:** yes

### [3] DBeaver-derived probes are co-located with attribution, no separate license file

- **Decision:** DBeaver-derived probes live under `tests/jdbc/upstream-mined/dbeaver/` with attribution comments in each probe; no separate LICENSE file is needed.
- **Alternatives:** Mirror Metabase's isolation for symmetry.
- **Rationale:** DBeaver Community is Apache 2.0, which is permissive and compatible with the repository's existing license posture. Isolation buys nothing here and adds friction.
- **Promotes to ADR:** yes

### [4] Per-tool baseline promotion via checked-in `baselines/<tool>.txt`

- **Decision:** Specify (do not implement) a per-tool baseline-promotion mechanism: `tests/jdbc/baselines/<tool>.txt` lists probe IDs pinned as regressions for that tool. The reporter classifies outcomes as `must-pass-failure`, `tool-baseline-failure`, `exploratory-failure`, or `pass`.
- **Alternatives:** Promote `EXPLORATORY` to `MUST_PASS` globally once a probe passes; annotation-based per-tool promotion in the source file.
- **Rationale:** A flat checked-in file matches the user's stated mental model ("pinned once it passes") with the minimum machinery, and keeps tool-baseline membership orthogonal to the existing `Expectation` enum.
- **Promotes to ADR:** yes

### [5] POC sized at ~5 probes per tool, all `EXPLORATORY`

- **Decision:** The proof-of-concept adds at least five mined probes per tool, all classified `EXPLORATORY` (cannot break must-pass).
- **Alternatives:** Larger POC; smaller POC.
- **Rationale:** Five probes each is enough to demonstrate the tooling, the provenance contract, and the AGPL-isolation flow without consuming budget that belongs to follow-up corpora plans.
- **Promotes to ADR:** no

### [6] Spec deltas attach to existing `testing/client-compatibility-harness` feature

- **Decision:** New scenarios (provenance, license-aware storage, ADR existence, POC, baseline-promotion specification) attach to the existing harness feature rather than spawning a new `upstream-test-corpus-provenance` feature.
- **Alternatives:** Create a new feature for upstream-test-corpus provenance and license handling.
- **Rationale:** All new behaviors are observable through the same harness, the same single-command entry point, and the same report. Spawning a new feature would fragment the harness contract without buying clarity.
- **Promotes to ADR:** no

### [7] Tasks 6 and 7 tagged `[expert]`

- **Decision:** The two mining tasks (DBeaver and Metabase) are tagged `[expert]`.
- **Alternatives:** Leave untagged.
- **Rationale:** The Metabase task requires non-obvious architectural care (AGPL-isolated directory, release-artifact exclusion, loader that loads only at test time). The DBeaver task is tagged for symmetry and because picking the right upstream files (the SQL strings can be assembled across multiple Java sources) requires careful reading rather than copy-paste from existing patterns.
- **Promotes to ADR:** no

### [8] Automated re-mining deferred entirely

- **Decision:** No work in this plan addresses automated re-mining as upstream tools update.
- **Alternatives:** Build a minimal re-miner now; specify the re-mining contract.
- **Rationale:** The user explicitly deferred this. The corpus is unstable until follow-up plans deliver the full corpora; re-mining design is premature.
- **Promotes to ADR:** no

## Review Findings

<!-- Populated by speq-implement after code review. -->
