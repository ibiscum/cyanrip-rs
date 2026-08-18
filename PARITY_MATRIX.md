# Parity Matrix (C -> Rust)

Last updated: 2026-08-18

Legend:
- Done: Implemented in Rust with regression tests.
- In progress: Partially implemented or scaffolded.
- Planned: Not started, design known.
- Deferred: Explicitly postponed until core path is stable.

## Scope Baseline

Source baseline is /home/ulf/data/cyanrip/src.

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
| Name templating | naming.c | template interpolation and directory creation behavior | src/naming.rs | Planned | No |
| Path sanitation | naming.c + os_compat.h | platform-sensitive replacement policy | src/naming.rs | Planned | No |
| CUE writer | cue_writer.c | CUE generation and track mapping details | src/cue.rs | Planned | No |
| Log formatter | cyanrip_log.c | report formatting, status lines, checksum sections | src/log_report.rs | Planned | No |
| FUN512 | fun512.c | SHA-512 + base64 marker digest for logs | src/fun512.rs | Planned | No |
| Disc ID generation | discid.c | MusicBrainz disc id and related tags | src/metadata/discid.rs | Planned | No |
| MusicBrainz metadata | musicbrainz.c | release lookup and metadata mapping | src/metadata/musicbrainz.rs | Planned | No |
| Cover art retrieval | coverart.c | Cover Art Archive querying/downloading | src/metadata/coverart.rs | Planned | No |
| AccurateRip lookup | accurip.c | AR DB download and checksum confidence matching | src/metadata/accurip.rs | Planned | No |
| Encoder pipeline | cyanrip_encode.c | decode/filter/encode/write pipeline | src/audio/* | Planned | No |
| FIFO frame/packet queues | fifo_frame.c + fifo_packet.c | thread-safe producer-consumer queues | src/audio/queue.rs | Planned | No |
| CD image + drive access | cyanrip_main.c + libcdio/paranoia | media read, retries, hot-remove checks | src/cdda/* | Planned | No |
| ReplayGain and EBU R128 | cyanrip_main.c + cyanrip_encode.c | album/track loudness metadata computation | src/audio/replaygain.rs | Deferred | No |
| Full codec parity set | cyanrip_encode.c | FLAC, MP3, TTA, OPUS, AAC, WV, VORBIS, ALAC, WAV, PCM | src/audio/codecs/* | Deferred | No |

## Current Gap Summary

- Implemented and test-covered: core settings and validation logic from CLI/control path.
- Major gaps: full CLI parser, naming/cue/log deterministic modules, metadata/network modules, audio pipeline, and CD I/O layer.
- Deferred explicitly: full codec parity and replaygain implementation details until core end-to-end path is stable.

## Immediate Next Slice

1. Implement src/cli.rs with Clap and map parsed args into Settings.
2. Add regression tests for representative valid and invalid command lines.
3. Keep error semantics aligned to this parity matrix and update statuses as features land.