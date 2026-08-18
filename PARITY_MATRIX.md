# Parity Matrix (C -> Rust)

Last updated: 2026-08-18

Legend:
- Done: Implemented in Rust with regression tests.
- In progress: Partially implemented or scaffolded.
- Planned: Not started, design known.
- Deferred: Explicitly postponed until core path is stable.

## Scope Baseline

Source baseline is /cyanrip/src.

## Feature Parity Table

| Area | C source | C behavior summary | Rust target | Status | Tests |
|---|---|---|---|---|---|
| Settings defaults | cyanrip_main.c | Initializes all default options and naming schemes | src/lib.rs Settings::default | Done | Yes |
| Device suffix dispatch | cyanrip_main.c | .bin/.cue/.nrg/.toc dispatch logic | src/lib.rs open_dev_kind | Done | Yes |
| Offset frame math | cyanrip_main.c | offset to over/under-read frame conversion | src/lib.rs calc_over_under_read_frames | Done | Yes |
| Paranoia parsing | cyanrip_main.c | none/max/int + bounds validation | src/lib.rs parse_paranoia | Done | Yes |
| Cover size validation | cyanrip_main.c | accept -1, 250, 500, 1200 | src/lib.rs parse_cover_size | Done | Yes |
| Sanitize mode validation | cyanrip_main.c | simple/os_simple/unicode/os_unicode | src/lib.rs parse_sanitize | Done | Yes |
| Disc tag parse | cyanrip_main.c | parse disc and totaldiscs with constraints | src/lib.rs parse_disc | Done | Yes |
| Release selector parse | cyanrip_main.c | numeric index or MB release id | src/lib.rs parse_release | Done | Yes |
| Output format validation | cyanrip_main.c | validate output names + duplicate detection | src/lib.rs parse_outputs | Done | Yes |
| Track index normalization | cyanrip_main.c | dedupe rejection + numeric sort | src/lib.rs parse_track_indices | Done | Yes |
| Pregap parse/apply | cyanrip_main.c | parse N=action and apply to per-track config | src/lib.rs parse_pregap_entry and apply_pregap_entries | Done | Yes |
| Folder scheme rule | cyanrip_main.c | require {format} when multiple outputs | src/lib.rs validate_folder_scheme | Done | Yes |
| Mode conflict rule | cyanrip_main.c | reject info-only and cue-only together | src/lib.rs validate_mode_combo | Done | Yes |
| Full CLI parser | genopt.h + cyanrip_main.c | full command-line flag and value parser | src/cli.rs | Done | Yes (golden + edge cases + help layout) |
| Name templating | naming.c | template interpolation and directory creation behavior | src/naming.rs | Done (core rules) | Yes |
| Path sanitation | naming.c + os_compat.h | platform-sensitive replacement policy | src/naming.rs | Done (core rules) | Yes |
| CUE writer | cue_writer.c | CUE generation and track mapping details | src/cue.rs | Done (core rules) | Yes |
| Log formatter | cyanrip_log.c | report formatting, status lines, checksum sections | src/log_report.rs | Done (deterministic sections) | Yes |
| FUN512 | fun512.c | SHA-512 + base64 marker digest for logs | src/fun512.rs | Done (core rules) | Yes |
| Disc ID generation | discid.c | MusicBrainz disc id, CDDB, and submission TOC URL generation | src/metadata/discid.rs | Done (core rules) | Yes |
| MusicBrainz metadata | musicbrainz.c | release lookup, selection semantics, and metadata mapping | src/metadata/musicbrainz.rs | Done (core rules) | Yes |
| Cover art retrieval | coverart.c | Cover Art Archive querying/downloading and selection policy | src/metadata/coverart.rs | Done (core rules) | Yes |
| AccurateRip lookup | accurip.c | AR DB download and checksum confidence matching | src/metadata/accurip.rs | Done (core rules) | Yes |
| Metadata flow orchestration | cyanrip_main.c + metadata modules | DiscID -> MB -> cover art -> AccurateRip ordering with disable/fallback behavior | src/app.rs | Done (core rules) | Yes |
| Encoder pipeline | cyanrip_encode.c | decode/filter/encode/write pipeline | src/audio/* | In progress (WAV+FLAC core paths) | Yes (WAV+FLAC) |
| WAV writer | cyanrip_encode.c | write PCM samples to RIFF/WAVE container | src/audio/wav.rs | Done (core rules) | Yes |
| FLAC writer | cyanrip_encode.c | write PCM samples to FLAC stream/container | src/audio/flac.rs | Done (core rules) | Yes |
| Per-track output dispatch | cyanrip_main.c + cyanrip_encode.c | select configured output formats and emit concrete per-track files | src/app.rs write_track_outputs | Done (WAV/FLAC scope) | Yes |
| FLAC metadata embedding | cyanrip_encode.c + cyanrip_main.c metadata flow | propagate album/track/disc metadata into FLAC Vorbis comments | src/app.rs write_track_outputs + metaflac | Done (FLAC scope) | Yes |
| FIFO frame/packet queues | fifo_frame.c + fifo_packet.c | thread-safe producer-consumer queues | src/audio/queue.rs | Planned | No |
| Paranoia ripping state machine | cyanrip_main.c + cdio/paranoia callbacks | retry loop, retry-limit finalize, media-changed abort, and flush/finalize transitions | src/cdda/paranoia.rs | In progress (control-path scaffold) | Yes |
| CD image + drive access | cyanrip_main.c + libcdio/paranoia | media read, retries, hot-remove checks | src/cdda/reader.rs + src/cdda/linux_drive.rs | In progress (trait + image-backed fake + Linux adapter scaffold) | Yes |
| ReplayGain and EBU R128 | cyanrip_main.c + cyanrip_encode.c | album/track loudness metadata computation | src/audio/replaygain.rs | Deferred | No |
| Full codec parity set | cyanrip_encode.c | FLAC, MP3, TTA, OPUS, AAC, WV, VORBIS, ALAC, WAV, PCM | src/audio/codecs/* | Deferred | No |

## Current Gap Summary

- Implemented and test-covered: core settings and validation logic from CLI/control path plus deterministic naming/cue/log/checksum modules, all M3 metadata core modules, and metadata-flow orchestration.
- Major gaps: metadata embedding for non-FLAC codecs, broader audio processing stages, and CD I/O backend integration.
- Paranoia mode status: control-path state machine, image-backed reader/fault-injection integration, and first Linux physical-drive adapter landed; live libcdio runtime validation is pending.
- Deferred explicitly: full codec parity and replaygain implementation details until core end-to-end path is stable.

## Immediate Next Slice

1. Validate libcdio-backed Linux adapter on real hardware and complete callback/paranoia parity checks.
2. Expand metadata embedding to additional codecs as they are implemented.
3. Keep unsupported codecs behind explicit deferred errors until implemented.
4. Keep error semantics aligned to this parity matrix and update statuses as features land.