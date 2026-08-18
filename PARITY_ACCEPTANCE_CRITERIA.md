# Parity Acceptance Criteria And Allowed Differences

Last updated: 2026-08-18

This document defines objective acceptance criteria for C-to-Rust parity and what differences are allowed at each stage.

## Purpose

- Prevent ambiguous parity decisions.
- Define measurable gates before moving between migration milestones.
- Separate critical parity from acceptable implementation differences.

## Scope

In scope:
- CLI defaults, validation semantics, and custom validation error strings.
- Deterministic outputs: naming, CUE text, log text, FUN512/checksum outputs.
- Metadata field mapping behavior.
- End-to-end behavior for implemented feature subsets.

Out of scope for strict byte parity:
- Underlying library internals (allocator behavior, thread scheduling, etc).
- Non-deterministic runtime details (timestamps, wall-clock pacing, hardware timing).

## Severity Levels

- Critical: user-visible behavior incompatibility that changes correctness, safety, or expected result set.
- Major: behavior difference that changes output shape/content but has a documented workaround.
- Minor: cosmetic formatting difference with no semantic impact.

Release acceptance for any milestone requires:
- zero Critical issues open,
- zero untracked Major issues,
- Minor issues either fixed or listed in known-differences notes.

## Global Acceptance Gates

A milestone is accepted only when all gates below pass:

1. Test gate
- All unit/integration/snapshot tests for the milestone pass in CI.
- No ignored tests for parity-critical paths unless explicitly justified in notes.

2. Fixture gate
- Relevant fixtures exist in tests/fixtures and are referenced by tests.
- Fixture updates require rationale in PR notes.

3. Contract gate
- PARITY_MATRIX.md status lines updated for affected features.
- CLI_BEHAVIOR_FREEZE.md and this document remain consistent with implementation.

4. Regression gate
- Existing passing parity tests remain green.
- New behavior must include at least one regression test.

## M0 Acceptance Criteria

### CLI parity (defaults/validation/errors)

Pass criteria:
- Rust behavior matches frozen expectations in CLI_BEHAVIOR_FREEZE.md.
- Custom validation error strings match exact frozen strings.
- Special-flow precedence matches frozen order:
- verify-log short-circuit first,
- outputs-help short-circuit second,
- normal validation otherwise.

Allowed differences:
- Clap-generated usage/help wrapping and spacing may differ by terminal width.
- Option ordering in generic Clap parser errors may differ.

Not allowed:
- Changes to frozen default values.
- Changes to frozen custom validation error strings.
- Changes to short-circuit precedence.

### Fixture readiness

Pass criteria:
- Sample fixtures exist for cue/log/naming/checksum domains in tests/fixtures.
- Fixtures are deterministic and parseable by tests.

Allowed differences:
- Placeholder sample metadata values are allowed as long as deterministic.

Not allowed:
- Missing fixture category for any M0 deterministic domain.

## M1 Acceptance Criteria

Pass criteria:
- CLI option coverage and mapping match target scope in PARITY_MATRIX.md.
- Golden C-style invocation tests pass.
- Exact custom validation errors covered by tests.

Allowed differences:
- Long-option alias expansion may include Rust-only convenience aliases.

Not allowed:
- Regressions in frozen CLI contract.

## M2 Acceptance Criteria

Pass criteria:
- Deterministic naming/cue/log/checksum outputs covered by snapshot or fixture tests.
- For each implemented deterministic module, at least one golden fixture comparison test passes.

Allowed differences:
- Whitespace-only differences where explicitly normalized before compare.

Not allowed:
- Semantically different tags/fields/index times/checksum strings without documented rationale.

## M3 Acceptance Criteria

Pass criteria:
- Metadata mapping tests pass using mocked network fixtures.
- Key fields in mapping table match expected values.

Allowed differences:
- HTTP header ordering and non-semantic transport differences.

Not allowed:
- Dropping required metadata fields without explicit deferred status.

## M4-M7 Acceptance Criteria (Progressive)

Pass criteria:
- Implemented codec/path subset has deterministic integration tests.
- Differential test harness compares Rust vs C outputs for shared scenarios.
- Known differences list is current and approved.

Allowed differences:
- Explicitly deferred codecs/features listed in PARITY_MATRIX.md.
- Performance variance not affecting correctness.

Not allowed:
- Silent behavior drift outside known differences.

## Allowed-Differences Policy

A difference is allowed only if all are true:
1. It is documented in PARITY_MATRIX.md or release notes.
2. It is classified as Minor or accepted Major.
3. A test asserts the intended normalized behavior where practical.
4. It does not violate any frozen contract file.

## Difference Triage Workflow

When C and Rust differ:
1. Classify severity (Critical/Major/Minor).
2. Decide: fix now, defer, or accept.
3. If defer/accept: document in PARITY_MATRIX.md with rationale.
4. Add/adjust tests to prevent accidental drift.

## Exit Condition For M0

M0 is complete only when:
- checklist items are checked in MIGRATION_PLAN.md,
- parity matrix exists,
- CLI behavior freeze exists,
- fixtures exist for cue/log/naming/checksum,
- this acceptance-criteria document exists and is linked from plan.
