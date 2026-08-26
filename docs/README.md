# Documentation Index

This folder tracks migration progress, completed changes, and upcoming implementation work.

Documents:
- [COMPLETED_STEPS.md](COMPLETED_STEPS.md): finished milestones, subtopics, and implementation highlights.
- [NEXT_STEPS.md](NEXT_STEPS.md): prioritized next tasks from the migration roadmap.
- [CHANGELOG.md](CHANGELOG.md): chronological log of migration changes in this Rust repository.
- [PARITY_NOTES.md](PARITY_NOTES.md): parity-focused notes and currently accepted differences.
- [ARCHITECTURE_CONTEXT_MAPPING.md](ARCHITECTURE_CONTEXT_MAPPING.md): mapping of upstream `cyanrip_ctx` responsibilities to Rust structures and flow boundaries.
- [OUTPUTROOT_OPTION_FLOW.md](OUTPUTROOT_OPTION_FLOW.md): resolution order and precedence details for `-B/--outputroot` and `CYANRIP_RS_OUTPUT_ROOT`.
- [ALBUM_META_OPTION_FLOW.md](album-meta_option_flow.md): purpose, parsing rules, precedence, and runtime behavior for `-a/--album-meta`.
- [DEVICE_OPTION_FLOW.md](device_option_flow.md): purpose and source-selection behavior for `-d/--device` across run, info-only, and find-offset modes.
- [OFFSET_OPTION_FLOW.md](offset_option_flow.md): sample-offset semantics, derived over/under-read frame behavior, and mode-specific runtime effects for `-s/--offset`.
- [RETRIES_OPTION_FLOW.md](retries_option_flow.md): retry-cap semantics and paranoia/repeat-rip wiring for `-r/--retries`.
- [REPEAT_RIPS_OPTION_FLOW.md](repeat-rips_option_flow.md): checksum-repeat goal semantics and retry-policy wiring for `-Z/--repeat-rips`.
- [SPEED_OPTION_FLOW.md](speed_option_flow.md): `-S/--speed` parsing, settings mapping, and current runtime limitation notes.
- [PREGAP_OPTION_FLOW.md](pregap_option_flow.md): per-track pregap action parsing and runtime application for `-p/--pregap`.
- [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md): practical real-drive reliability scenarios and acceptance-notes template for M6 closure.
- [M7_DIFFERENTIAL_HARNESS.md](M7_DIFFERENTIAL_HARNESS.md): first-slice differential testing against the C binary and M7 expansion steps.

Primary planning references outside this folder:
- [../MIGRATION_PLAN.md](../MIGRATION_PLAN.md)
- [../PARITY_MATRIX.md](../PARITY_MATRIX.md)
- [../CLI_BEHAVIOR_FREEZE.md](../CLI_BEHAVIOR_FREEZE.md)
- [../PARITY_ACCEPTANCE_CRITERIA.md](../PARITY_ACCEPTANCE_CRITERIA.md)
