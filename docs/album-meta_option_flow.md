# `-a/--album-meta` Option Flow

Last updated: 2026-08-26

This document explains why `--album-meta` exists, how to format it, and where it is applied in runtime flows.

## Purpose

`--album-meta` lets you provide album-level metadata directly from CLI, without requiring metadata lookup services.

Typical use cases:
- Override MusicBrainz-derived album fields (for example, custom album title/artist/date).
- Provide deterministic naming inputs for folder/file templates.
- Provide FLAC tag values when ripping with local-only or partially available metadata.

## CLI Format

Option:
- `-a` or `--album-meta`

Value format:
- `key=value:key=value`

Examples:

```bash
cyanrip-rs -a "album=Kind of Blue:album_artist=Miles Davis:date=1959"
```

```bash
cyanrip-rs --album-meta "album=Live Set:releasecomment=Remaster"
```

Parsing behavior:
- Entries are split by `:` into pairs.
- Each pair is split once on `=` into `key` and `value`.
- Empty keys or empty values are ignored.
- Whitespace around keys/values is trimmed.

## Flow Overview

1. CLI parse
- `src/cli.rs` defines `--album-meta` and stores raw text in `settings.album_metadata`.

2. Runtime parse
- `src/app.rs` parses `settings.album_metadata` into a map via `parse_album_metadata_map`.

3. Consumption points
- CUE-only preview uses album metadata map for CUE document-level metadata.
- Full-rip bridge uses album metadata map as input to naming/template resolution.
- FLAC tag embedding merges album metadata into Vorbis comments.

## Precedence Rules

### Full-rip naming/tag baseline

In full-rip and synthetic flows, album metadata from `--album-meta` is loaded first. Metadata discovered at runtime (for example from MusicBrainz) is then applied with `or_insert` semantics, so user-provided keys win when both provide the same key.

Practical effect:
- `--album-meta album=...` overrides discovered album title for naming/tagging inputs.
- Missing keys can still be filled by discovered metadata or runtime defaults.

### CUE runtime metadata

For runtime CUE metadata assembly, release metadata is populated first, then selected user keys from `--album-meta` (`album`, `album_artist`, `date`) overwrite those fields when present.

## Key Behavior Notes

- `--album-meta` is album-level only; per-track overrides belong to `--track-meta`.
- Unknown keys are preserved in the internal metadata map and can still influence template/tag paths where those keys are consumed.
- FLAC tag keys are canonicalized to uppercase/Vorbis-style names (for example `album_artist` -> `ALBUMARTIST`, `musicbrainz_albumid` -> `MUSICBRAINZ_ALBUMID`).

## Related Options

- `--track-meta`: per-track metadata entries.
- `--release`: release selection when multiple MusicBrainz releases are available.
- `--no-musicbrainz`: disables MusicBrainz lookup, making user-provided metadata more central.

## Regression Coverage

- CLI mapping test: `src/cli.rs` (`golden_c_style_metadata_invocation`)
- Output writer/tag flow coverage: `tests/app_cli_integration.rs` (`cli_outputs_and_disc_tags_drive_writer_dispatch_and_flac_tags`)
