# Parity Matrix (C -> Rust)

Last updated: 2026-09-11

Legend:
- Complete: Implemented in Rust with regression tests.
- In progress: Partially implemented or scaffolded.
- Planned: Not started, design known.
- Deferred: Explicitly postponed until core path is stable.

## Scope Baseline

Source baseline is /cyanrip/src.

## Feature Parity Table

| Area | C source | C behavior summary | Rust target | Status | Tests |
|---|---|---|---|---|---|
| Settings defaults | cyanrip_main.c | Initializes all default options and naming schemes | src/lib.rs Settings::default | Complete | Yes |
| Device suffix dispatch | cyanrip_main.c | .bin/.cue/.nrg/.toc dispatch logic | src/lib.rs open_dev_kind | Complete | Yes |
| Offset frame math | cyanrip_main.c | offset to over/under-read frame conversion | src/lib.rs calc_over_under_read_frames | Complete | Yes |
| Paranoia parsing | cyanrip_main.c | none/max/int + bounds validation | src/lib.rs parse_paranoia | Complete | Yes |
| Cover size validation | cyanrip_main.c | accept -1, 250, 500, 1200 | src/lib.rs parse_cover_size | Complete | Yes |
| Sanitize mode validation | cyanrip_main.c | simple/os_simple/unicode/os_unicode | src/lib.rs parse_sanitize | Complete | Yes |
| Disc tag parse | cyanrip_main.c | parse disc and totaldiscs with constraints | src/lib.rs parse_disc | Complete | Yes |
| Release selector parse | cyanrip_main.c | numeric index or MB release id | src/lib.rs parse_release | Complete | Yes |
| Output format validation | cyanrip_main.c | validate output names + duplicate detection | src/lib.rs parse_outputs | Complete | Yes |
| Track index normalization | cyanrip_main.c | dedupe rejection + numeric sort | src/lib.rs parse_track_indices | Complete | Yes |
| Pregap parse/apply | cyanrip_main.c | parse N=action and apply to per-track config | src/lib.rs parse_pregap_entry and apply_pregap_entries | Complete | Yes |
| Folder scheme rule | cyanrip_main.c | require {format} when multiple outputs | src/lib.rs validate_folder_scheme | Complete | Yes |
| Mode conflict rule | cyanrip_main.c | reject info-only and cue-only together; reject find-offset with info-only/cue-only | src/lib.rs validate_mode_combo | Complete | Yes |
| Full CLI parser | genopt.h + cyanrip_main.c | full command-line flag and value parser | src/cli.rs | Complete | Yes (golden + edge cases + help layout) |
| Verify-log action dispatch | cyanrip_main.c + fun512.c | verify-log short-circuit parse path and checksum status mapping | src/cli.rs + src/main.rs + src/fun512.rs | Complete | Yes |
| Eject option runtime behavior | cyanrip_main.c | success-path cleanup eject for capable physical drives; parser side effects disable in info/find-offset modes | src/cli.rs + src/app.rs + src/cdda/linux_drive.rs | Complete (Linux libcdio-sys scope) | Yes |
| No-coverart-embed runtime behavior | cyanrip_encode.c | disable embedded cover art while keeping cover discovery and standalone cover-file behavior | src/cli.rs + src/app.rs | Complete (FLAC scope) | Yes |
| Name templating | naming.c | template interpolation, conditional expansion, path-kind builders (track/log/cue/cover), collision checks, optional parent-dir creation, and multi-disc default track prefix parity (`disc.track`) via merged album+track tag evaluation | src/naming.rs + src/app.rs | Complete | Yes |
| Path sanitation | naming.c + os_compat.h | platform-sensitive replacement policy | src/naming.rs | Complete | Yes |
| CUE writer | cue_writer.c | CUE generation and track mapping details (metadata lines, per-track FILE/TRACK records, pregap/index handling, preemphasis/ISRC plus SONGWRITER/COMPOSER/ARRANGER, FLAGS PRE/DCP/4CH/SCMS, POSTGAP, and cue-path-relative filenames) | src/cue.rs + src/app.rs cue-track ingestion | Complete | Yes |
| Log formatter | cyanrip_log.c | report formatting, status lines, checksum sections | src/log_report.rs | Complete (deterministic sections) | Yes |
| FUN512 | fun512.c | SHA-512 + base64 marker digest for logs | src/fun512.rs | Complete (core rules) | Yes |
| Disc ID generation | discid.c | MusicBrainz disc id, CDDB, and submission TOC URL generation | src/metadata/discid.rs | Complete (core rules; `mb_submission_url` produced from TOC) | Yes |
| MusicBrainz metadata | musicbrainz.c | release lookup, selection semantics, and metadata mapping; when no release is found, present the TOC-based MusicBrainz submission URL | src/metadata/musicbrainz.rs | Complete (core rules; NotFound path now emits the submission link alongside the lookup failure warning) | Yes |
| Cover art retrieval | coverart.c | Cover Art Archive querying/downloading and selection policy | src/metadata/coverart.rs | Complete (core rules) | Yes |
| AccurateRip lookup | accurip.c | AR DB download and checksum confidence matching | src/metadata/accurip.rs | Complete (core rules) | Yes |
| AccurateRip checksum verification in rip path | cyanrip_main.c + checksums.h + cyanrip_log.c | compute per-track AccurateRip v1/v2 checksums from ripped audio and match with DB confidences; per-track result emitted during rip | src/app.rs + src/log_report.rs + src/metadata/accurip.rs | Complete (v1/v2 checksums from drive-offset-corrected PCM; per-track verified/confidence-0/unavailable/mismatch messages emitted during rip; retry-on-mismatch removed to match upstream) | Yes |
| AccurateRip finish-summary parity | cyanrip_log.c | emit "Tracks ripped accurately" and "Tracks ripped partially accurately" based on per-track AR matches | src/log_report.rs + src/app.rs | In progress (formatter exists; runtime full-rip bridge emits per-track verification messages but does not yet populate finish-summary aggregate counts) | No |
| Per-track rip summary | cyanrip_log.c + cyanrip_main.c | EBU R128 loudness, EAC CRC32, AccurateRip v1/v2, preemphasis, LSN/duration/properties, metadata, embedded cover art, written files | src/app.rs + src/audio/loudness.rs + src/fun512.rs | Complete (upstream-style Summary block printed after each track and appended to log; EBU R128 integrated loudness/LRA/true peak/sample peak; EAC CRC32; AccuRip v1/v2; preemphasis flag; ReplayGain/R128 gain tags in summary metadata) | Yes |
| Metadata flow orchestration | cyanrip_main.c + metadata modules | DiscID -> MB -> cover art -> AccurateRip ordering with disable/fallback behavior | src/app.rs | Complete (core rules) | Yes |
| Full-rip happy-path orchestration parity | cyanrip_main.c | start-report -> track offset/pregap setup -> album-to-track metadata copy -> track-coverart fill -> rip loop ordering | src/app.rs | In progress (bridge path runs metadata + rip/write, but upstream orchestration stages are not fully mirrored) | No |
| Encoder pipeline | cyanrip_encode.c | decode/filter/encode/write pipeline | src/audio/* | In progress (WAV+FLAC core paths + option-driven processing stage) | Yes (WAV+FLAC) |
| WAV writer | cyanrip_encode.c | write PCM samples to RIFF/WAVE container | src/audio/wav.rs | Complete (core rules) | Yes |
| FLAC writer | cyanrip_encode.c | write PCM samples to FLAC stream/container | src/audio/flac.rs | Complete (core rules) | Yes |
| Per-track output dispatch | cyanrip_main.c + cyanrip_encode.c | select configured output formats and emit concrete per-track files | src/app.rs write_track_outputs | Complete (WAV/FLAC scope) | Yes |
| FLAC metadata embedding | cyanrip_encode.c + cyanrip_main.c metadata flow | propagate album/track/disc metadata into FLAC Vorbis comments and attach cover art when enabled | src/app.rs write_track_outputs + metaflac | Complete (FLAC scope) | Yes |
| HDCD/deemphasis option handling | cyanrip_encode.c + cyanrip_main.c | processing-path selection precedence: HDCD over deemphasis; -W disables auto-deemphasis; -E forces deemphasis unless HDCD path is selected | src/audio/process.rs + src/app.rs | Complete (ffmpeg hdcd backend wired; 24-bit output propagation for WAV/FLAC) | Yes |
| FIFO frame/packet queues | fifo_frame.c + fifo_packet.c | thread-safe producer-consumer queues | src/audio/queue.rs | Planned | No |
| Paranoia ripping state machine | cyanrip_main.c + cdio/paranoia callbacks | retry loop, retry-limit finalize, media-changed abort, and flush/finalize transitions | src/cdda/paranoia.rs + src/cdda/reader.rs + src/app.rs | In progress (state machine wired and consumed by physical full-rip; precheck-plus-direct-read split removed for paranoia-enabled paths) | Yes |
| CD image + drive access | cyanrip_main.c + libcdio/paranoia | media read, retries, hot-remove checks | src/cdda/reader.rs + src/cdda/linux_drive.rs + src/app.rs | In progress (image-backed + Linux adapters wired; one native paranoia session reader is now opened per physical rip and reused across tracks, matching upstream `ctx->paranoia` lifetime; broader real-drive parity still pending) | Yes |
| ReplayGain and EBU R128 | cyanrip_main.c + cyanrip_encode.c | album/track loudness metadata computation | src/app.rs FLAC tag flow + src/audio/loudness.rs | In progress (EBU R128 loudness values computed and shown in per-track summary; FLAC tag embedding still uses RMS-based ReplayGain approximation; EBU R128-based tag embedding pending) | Yes (FLAC scope) |
| Full codec parity set | cyanrip_encode.c | FLAC, MP3, TTA, OPUS, AAC, WV, VORBIS, ALAC, WAV, PCM | src/audio/codecs/* | Deferred (out of current scope: FLAC-only target) | No |

## Current Gap Summary

- Implemented and test-covered: core CLI/control behavior, deterministic naming/CUE/log/checksum modules, metadata services/orchestration, and functional full-rip execution paths (image + linux physical scope) through acquisition, processing, naming, writing, and per-track summary output.
- Remaining major gaps are parity-completeness items, not baseline ripping availability: real-hardware edge-case validation breadth, some finish-summary aggregation parity, and remaining tagging/reporting deltas.
- Paranoia mode status: physical full-rip uses the integrated paranoia reader path (single native session reused across tracks) and consumes paranoia-produced frames directly. Remaining work is callback/status parity closure and broader real-hardware edge-case coverage.
- AccurateRip rip-time status: per-track v1/v2 verification messages are emitted during ripping (`verified`, `confidence is 0`, `verification unavailable`, `mismatch`), with mismatch retry behavior aligned to upstream (no auto retry-on-mismatch). Remaining gap: aggregate finish-summary counts are still not populated.
- Per-track summary status: upstream-style `Summary:` is printed after each encoded track and appended to the runtime log, including EBU R128 loudness metrics, EAC CRC32, AccuRip v1/v2, preemphasis, track properties, metadata, cover-art entries, and written file paths.
- Deferred explicitly: full multi-codec parity remains out of scope; FLAC-path replaygain tag embedding still uses RMS-based approximation while EBU R128 values are already available in runtime summary output.

## Immediate Next Slice

1. Expand real-hardware validation matrix for linux libcdio/paranoia runs and close remaining callback/status parity checks.
2. Implement AccurateRip finish-summary aggregate counts (`Tracks ripped accurately` / `partially accurately`) in the runtime bridge output.
3. Align FLAC tag embedding with EBU R128-based gain values (keeping existing summary output behavior).
4. Keep deferred codec boundaries explicit and continue updating matrix status lines as each remaining parity gap closes.