# Documentation Index

This folder tracks migration progress, completed changes, and upcoming implementation work.

Documents:
- [COMPLETED_STEPS.md](COMPLETED_STEPS.md): finished milestones, subtopics, and implementation highlights.
- [NEXT_STEPS.md](NEXT_STEPS.md): prioritized next tasks from the migration roadmap.
- [CHANGELOG.md](CHANGELOG.md): chronological log of migration changes in this Rust repository.
- [PARITY_NOTES.md](PARITY_NOTES.md): parity-focused notes and currently accepted differences.
- [ARCHITECTURE_CONTEXT_MAPPING.md](ARCHITECTURE_CONTEXT_MAPPING.md): mapping of upstream `cyanrip_ctx` responsibilities to Rust structures and flow boundaries.
- [OUTPUTROOT_OPTION_FLOW.md](OUTPUTROOT_OPTION_FLOW.md): resolution order and precedence details for `-B/--output-root` and `CYANRIP_RS_OUTPUT_ROOT`.
- [OUTPUTS_OPTION_FLOW.md](outputs_option_flow.md): purpose, parsing/validation, runtime dispatch flow, and implementation status for `-o/--outputs`.
- [ALBUM_META_OPTION_FLOW.md](album-meta_option_flow.md): purpose, parsing rules, precedence, and runtime behavior for `-a/--album-meta`.
- [TRACK_META_OPTION_FLOW.md](track-meta_option_flow.md): purpose, parse/merge flow, runtime consumption points, and implementation status for `-t/--track-meta`.
- [RELEASE_OPTION_FLOW.md](release_option_flow.md): purpose, selection/disambiguation flow, and implementation status for `-R/--release`.
- [DISC_OPTION_FLOW.md](disc_option_flow.md): purpose, parse/validation flow, runtime metadata consumption, and implementation status for `-c/--disc`.
- [COVER_OPTION_FLOW.md](cover_option_flow.md): purpose, parse/validation rules, runtime cover staging/writing flow, and implementation status for `-C/--cover`.
- [NO_COVERART_OPTION_FLOW.md](no-coverart_option_flow.md): umbrella behavior mapping and implementation status for `--no-coverart` versus `--no-coverart-db` and `--no-coverart-embed`.
- [NO_COVERART_EMBED_OPTION_FLOW.md](no-coverart-embed_option_flow.md): purpose, FLAC embedding-gate flow, and implementation status for `-G/--no-coverart-embed`.
- [COVER_SIZE_OPTION_FLOW.md](cover-size_option_flow.md): purpose, size-variant lookup flow, and implementation status for `-m/--cover-size`.
- [DEVICE_OPTION_FLOW.md](device_option_flow.md): purpose and source-selection behavior for `-d/--device` across run, info-only, and find-offset modes.
- [EJECT_OPTION_FLOW.md](eject_option_flow.md): purpose, success-path cleanup flow, and implementation status for `-Q/--eject`.
- [INFO_OPTION_FLOW.md](info_option_flow.md): purpose, mode-dispatch flow, report behavior, and implementation status for `-I/--info`.
- [VERIFY_LOG_OPTION_FLOW.md](verify-log_option_flow.md): purpose, action-dispatch flow, and implementation status for `-Y/--verify-log`.
- [CUE_ONLY_OPTION_FLOW.md](cue-only_option_flow.md): purpose, dispatch/offset-guard flow, and implementation status for `-J/--cue-only`.
- [NO_ACCURIP_OPTION_FLOW.md](no-accurip_option_flow.md): purpose, AccurateRip-gating flow, and implementation status for `-A/--no-accurip`.
- [OFFSET_OPTION_FLOW.md](offset_option_flow.md): sample-offset semantics, derived over/under-read frame behavior, and mode-specific runtime effects for `-s/--offset`.
- [RETRIES_OPTION_FLOW.md](retries_option_flow.md): retry-cap semantics and paranoia/repeat-rip wiring for `-r/--retries`.
- [REPEAT_RIPS_OPTION_FLOW.md](repeat-rips_option_flow.md): checksum-repeat goal semantics and retry-policy wiring for `-Z/--repeat-rips`.
- [TRACKS_OPTION_FLOW.md](tracks_option_flow.md): purpose, parse/normalization flow, runtime track-selection behavior, and implementation status for `-l/--tracks`.
- [SANITIZE_OPTION_FLOW.md](sanitize_option_flow.md): purpose, sanitize-mode parse flow, naming-path runtime effects, and implementation status for `-T/--sanitize`.
- [BITRATE_OPTION_FLOW.md](bitrate_option_flow.md): purpose, parse/settings flow, and implementation status for `-b/--bitrate`.
- [FOLDER_SCHEME_OPTION_FLOW.md](folder-scheme_option_flow.md): purpose, template validation flow, runtime naming usage, and implementation status for `-D/--folder-scheme`.
- [TRACK_SCHEME_OPTION_FLOW.md](track-scheme_option_flow.md): purpose, runtime filename rendering flow, and implementation status for `-F/--track-scheme`.
- [LOG_SCHEME_OPTION_FLOW.md](log-scheme_option_flow.md): purpose, log-path resolution flow, output-root precedence, and implementation status for `-L/--log-scheme`.
- [CUE_SCHEME_OPTION_FLOW.md](cue-scheme_option_flow.md): purpose, cue-path resolution flow, output-root precedence, and implementation status for `-M/--cue-scheme`.
- [SPEED_OPTION_FLOW.md](speed_option_flow.md): `-S/--speed` parsing, settings mapping, and current runtime limitation notes.
- [PREGAP_OPTION_FLOW.md](pregap_option_flow.md): per-track pregap action parsing and runtime application for `-p/--pregap`.
- [PARANOIA_OPTION_FLOW.md](paranoia_option_flow.md): level parsing and runtime reader/paranoia execution behavior for `-P/--paranoia`.
- [PARANOIA_UPSTREAM_PARITY_PLAN.md](paranoia_upstream_parity_plan.md): implementation-ready plan to move from precheck-plus-reread behavior to an upstream-style integrated paranoia loop.
- [HDCD_OPTION_FLOW.md](hdcd_option_flow.md): purpose, precedence, processing backend, and 24-bit output behavior for `-H/--hdcd`.
- [FORCE_DEEMPHASIS_OPTION_FLOW.md](force-deemphasis_option_flow.md): purpose, precedence, and runtime processing behavior for `-E/--force-deemphasis`.
- [NO_DEEMPHASIS_OPTION_FLOW.md](no-deemphasis_option_flow.md): purpose, automatic-path disable semantics, and runtime behavior for `-W/--no-deemphasis`.
- [NO_MUSICBRAINZ_OPTION_FLOW.md](no-musicbrainz_option_flow.md): purpose, metadata-enrichment gating flow, and implementation status for `-N/--no-musicbrainz`.
- [NO_REPLAYGAIN_OPTION_FLOW.md](no-replaygain_option_flow.md): purpose, FLAC ReplayGain tag gating behavior, and implementation status for `-K/--no-replaygain`.
- [M6_REAL_HARDWARE_VALIDATION.md](M6_REAL_HARDWARE_VALIDATION.md): practical real-drive reliability scenarios and acceptance-notes template for M6 closure.
- [M7_DIFFERENTIAL_HARNESS.md](M7_DIFFERENTIAL_HARNESS.md): first-slice differential testing against the C binary and M7 expansion steps.
- [logging.md](logging.md): diagnostic logging facility (`log`/`env_logger`), `RUST_LOG` verbosity control, and the split between logged diagnostics and protocol-output console text.

Primary planning references outside this folder:
- [../MIGRATION_PLAN.md](../MIGRATION_PLAN.md)
- [../PARITY_MATRIX.md](../PARITY_MATRIX.md)
- [../CLI_BEHAVIOR_FREEZE.md](../CLI_BEHAVIOR_FREEZE.md)
- [../PARITY_ACCEPTANCE_CRITERIA.md](../PARITY_ACCEPTANCE_CRITERIA.md)
