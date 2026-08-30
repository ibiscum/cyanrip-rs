# `-N/--no-musicbrainz` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--no-musicbrainz`, how it maps into runtime settings, and the current implementation status in cyanrip-rs.

## Purpose

`--no-musicbrainz` disables MusicBrainz release lookup.

In cyanrip-rs, MusicBrainz metadata enrichment is enabled by default when a disc ID is available. This option disables that network lookup path and keeps the run on local/TOC-derived metadata.

## CLI Surface

Options:
- `-N`
- `--no-musicbrainz`

Alias:
- `--no_musicbrainz`

Value type:
- boolean flag (disabled by default)

Examples:

```bash
cyanrip-rs -N -I -d /dev/cdrom
```

```bash
cyanrip-rs --no-musicbrainz -o wav,flac -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--no-musicbrainz` sets `CliArgs.no_musicbrainz = true`.
2. `CliArgs::to_config` maps this to `settings.disable_mb = true`.

Related side effect in another mode:

- `--find-offset` also forces `settings.disable_mb = true` even without `-N`, because find-offset mode does not use MusicBrainz metadata enrichment.

## Runtime Flow

Metadata orchestration gates MusicBrainz lookup on `settings.disable_mb`:

1. Disc ID is computed when possible.
2. If `settings.disable_mb == false`, the app performs MusicBrainz release lookup and can enrich album/track metadata.
3. If `settings.disable_mb == true` (via `-N` or forced by find-offset mode), MusicBrainz lookup is skipped and no MusicBrainz release metadata is attached.

This affects both normal rip workflows and info-only metadata reporting flows that rely on shared metadata orchestration.

## Output Impact

With MusicBrainz enabled (default):
- release enrichment can populate fields such as release title/date/label details and MusicBrainz IDs.

With `--no-musicbrainz`:
- MusicBrainz network lookup is skipped.
- MusicBrainz-derived fields are absent.
- local disc/track context (TOC-derived metadata and user-supplied overrides) still applies.

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration for `-N/--no-musicbrainz` plus alias `--no_musicbrainz`.
- Parse-to-settings mapping: `no_musicbrainz -> settings.disable_mb`.
- Runtime gating that skips MusicBrainz lookup when disabled.
- Integration coverage asserting no MusicBrainz calls occur when `-N` is set.
- Help text exposure in `-h/--help` output.

Known limits:

- Skipping MusicBrainz naturally removes release-enrichment data; this is expected behavior, not a parity gap.
- Runtime behavior still depends on selected mode/build features for what non-MusicBrainz metadata is available.

## Regression Coverage

Coverage includes:

- `src/cli.rs` tests that assert `-N` maps to `settings.disable_mb = true`.
- `tests/app_cli_integration.rs`:
  - `cli_disable_flags_propagate_to_metadata_orchestration` (verifies MusicBrainz call count is zero with `-N`).
- `src/app.rs` unit tests for metadata orchestration paths with `disable_mb` enabled.