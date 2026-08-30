# `-G/--no-coverart-embed` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--no-coverart-embed`, how it is parsed and mapped, and what behavior is currently implemented at runtime.

## Purpose

`--no-coverart-embed` disables embedding cover art into encoded output files.

It does not disable cover-art discovery/fetching and does not disable standalone cover image file output.

## CLI Surface

Options:
- `-G`
- `--no-coverart-embed`

Alias:
- `--no_coverart_embed`

Value type:
- boolean flag (disabled by default)

Examples:

```bash
cyanrip-rs -G -o flac -d disc.cue
```

```bash
cyanrip-rs --no-coverart-embed --cover "Front=/path/front.jpg" -o flac
```

## Parse and Settings Mapping

During CLI parse:

1. `--no-coverart-embed` sets `CliArgs.no_coverart_embed = true`.
2. `CliArgs::to_config` maps it to `settings.disable_coverart_embedding = true`.

Default behavior remains embedding enabled (`settings.disable_coverart_embedding = false`).

## Runtime Flow

Current FLAC writer behavior:

1. Cover-art candidates are staged/resolved upstream in metadata orchestration.
2. Track output flow computes an optional FLAC embedded picture payload from available cover art.
3. If `settings.disable_coverart_embedding == true`, the embedded picture payload is forced to `None`.
4. FLAC tagging writes Vorbis comments as usual and only writes a FLAC picture block when an embedded payload is present.

This mirrors upstream intent: `-G` gates embedding, not cover discovery or cover-file writing.

## Output Impact

With embedding enabled (default):
- FLAC outputs can contain embedded cover art (typically front cover when available).

With `--no-coverart-embed`:
- FLAC outputs keep metadata comments but omit embedded cover picture blocks.
- External cover image files are still handled by cover-file writing flow.

WAV outputs are unaffected because they do not use FLAC picture blocks.

## Interaction With Related Options

- `--no-coverart-db`: controls DB lookup/fetch; independent from embedding gate.
- `--cover`: still stages user-supplied cover inputs; `-G` only controls whether those covers are embedded.
- `--cover-size`: affects Cover Art DB variant selection, not embedding on/off.

## Implementation Status

Status: complete (current runtime scope)

Implemented now:

- CLI declaration and parse-to-settings mapping for `-G/--no-coverart-embed`.
- Runtime embedding gate in FLAC tag-writing path (`settings.disable_coverart_embedding`).
- Regression tests validating:
  - default behavior embeds FLAC cover art when available,
  - `-G` disables FLAC picture embedding.

Known limits:

- Embedding behavior is implemented for the currently active FLAC output path.
- As with the broader runtime, non-FLAC/non-active codec parity remains outside current scope.

## Regression Coverage

Coverage includes:

- `src/cli.rs` parse/mapping test assertions for `disable_coverart_embedding`.
- `src/app.rs` tests:
  - `flac_embeds_cover_art_by_default_when_available`
  - `no_coverart_embed_skips_flac_picture_embedding`