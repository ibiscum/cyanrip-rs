# cyanrip-rs

Step-by-step migration roadmap is documented in [MIGRATION_PLAN.md](MIGRATION_PLAN.md).
Feature-by-feature parity tracking is documented in [PARITY_MATRIX.md](PARITY_MATRIX.md).

## Update Workflow

For every PR, update docs as part of the same change set:

1. Always update [docs/CHANGELOG.md](docs/CHANGELOG.md) with a short entry describing what changed.
2. If milestone/checklist status changes, update [MIGRATION_PLAN.md](MIGRATION_PLAN.md) and [docs/COMPLETED_STEPS.md](docs/COMPLETED_STEPS.md).
3. If parity scope, status, or accepted differences change, update [PARITY_MATRIX.md](PARITY_MATRIX.md) and [docs/PARITY_NOTES.md](docs/PARITY_NOTES.md).
4. If CLI defaults/validation/errors or special-flow behavior changes, update [CLI_BEHAVIOR_FREEZE.md](CLI_BEHAVIOR_FREEZE.md) and related CLI tests.
5. If parity gates or allowed-difference policy changes, update [PARITY_ACCEPTANCE_CRITERIA.md](PARITY_ACCEPTANCE_CRITERIA.md).
6. If new deterministic sample data is added or changed, update files under [tests/fixtures/README.md](tests/fixtures/README.md).
7. If priorities shift, update [docs/NEXT_STEPS.md](docs/NEXT_STEPS.md).

PRs that modify behavior without corresponding doc updates should be considered incomplete.