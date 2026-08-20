# cyanrip-rs

This repository is an attempt to port from C to Rust with the help of GitHub Co-Pilot. Basis for this is the excellent application cyanrip which can be found here: https://github.com/cyanreg/cyanrip

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

## Linux CDDA Real-Drive Validation

The Linux physical-drive adapter is available behind the `backend-libcdio-sys` feature.

1. Verify native/runtime prerequisites:
	- `./scripts/check_linux_cdda_stack.sh`
2. Run the real-drive smoke test (ignored by default because hardware is required):
	- `CYANRIP_CDROM_DEVICE=/dev/sr0 cargo test --features "backend-libcdio-sys paranoia" --test linux_physical_drive_validation -- --ignored`

Notes:
- Use the actual device path for your machine (`/dev/sr0`, `/dev/cdrom`, etc.).
- Insert a readable audio CD beforehand (the first regression test reads TOC data from the drive).
- For practical M6 reliability scenario runs and acceptance-note capture, use `./scripts/run_m6_hardware_validation.sh` and [docs/M6_REAL_HARDWARE_VALIDATION.md](docs/M6_REAL_HARDWARE_VALIDATION.md).

## Licensing

- Project license: LGPL-2.1-or-later (see [LICENSE](LICENSE)).
- Upstream attribution and notice handling: [UPSTREAM_NOTICES.md](UPSTREAM_NOTICES.md).
- Third-party dependency license inventory: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
- Bundled common dependency license texts:
	- [licenses/Apache-2.0.txt](licenses/Apache-2.0.txt)
	- [licenses/MIT.txt](licenses/MIT.txt)
	- [licenses/ISC.txt](licenses/ISC.txt)