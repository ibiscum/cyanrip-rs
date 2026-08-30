# `-c/--disc` Option Flow

Last updated: 2026-08-28

This document explains the purpose of `--disc`, how it is parsed and mapped, and how it is consumed by runtime metadata flows in cyanrip-rs.

## Purpose

`--disc` provides explicit multi-disc context as `discnumber/totaldiscs`.

It is used to:

- inform metadata selection for multi-disc MusicBrainz releases,
- populate disc-level tags/fields used in output metadata and naming.

## CLI Surface

Options:

- `-c`
- `--disc`

Value type:

- `discnumber` or `discnumber/totaldiscs`

Examples:

```bash
cyanrip-rs -c 2/3 -I -d /dev/cdrom
```

```bash
cyanrip-rs --disc 1/2 -o flac -d disc.cue
```

## Parse and Settings Mapping

During CLI parse:

1. `--disc` raw value is parsed by `parse_disc`.
2. Parser validates value constraints:
   - `discnumber > 0`
   - `totaldiscs > 0` when provided
   - `discnumber <= totaldiscs` when both are provided
3. Parsed values are stored into:
   - `settings.discnumber`
   - `settings.totaldiscs`

Parse failures return explicit error messages (for example invalid zero values or `discnumber` greater than `totaldiscs`).

## Runtime Flow

### MusicBrainz release mapping context

When MusicBrainz lookup is active, `settings.discnumber` is passed into release selection/mapping logic in info and cue-only flows.

This helps select/validate the appropriate medium for multi-disc releases.

### Metadata map propagation

Album metadata construction can include disc-related fields (`disc`, `totaldiscs`) and feed naming/template expansion.

### FLAC metadata embedding

For FLAC output, disc fields are written when set:

- `DISCNUMBER` from `settings.discnumber`
- `DISCTOTAL` from `settings.totaldiscs`

## Interaction With Related Options

- `--release` works with `--disc` to disambiguate multi-disc releases.
- naming schemes using `{disc}` and `{totaldiscs}` can reflect these values in filenames/path components.
- if MusicBrainz provides release disc data, runtime metadata merge rules may also provide these fields.

## Implementation Status

Status: complete (current metadata and tagging scope)

Implemented now:

- CLI declaration for `-c/--disc`.
- parse validation and settings mapping for `discnumber/totaldiscs`.
- runtime consumption in MusicBrainz selection flows (info/cue-only and metadata paths).
- propagation to FLAC Vorbis tags (`DISCNUMBER`, `DISCTOTAL`) when configured.
- parser and runtime coverage for valid and invalid cases.

Known limits:

- behavior that depends on remote release data remains subject to MusicBrainz availability and build/runtime feature context.

## Regression Coverage

Coverage includes:

- `src/lib.rs`:
  - `disc_parse_regression`
- `src/cli.rs`:
  - `parses_extended_fields`
  - `golden_c_style_full_rip_invocation`
  - `rejects_invalid_disc_with_exact_message`
- `src/metadata/musicbrainz.rs`:
  - multi-disc mapping and invalid-discnumber handling tests in release lookup flow
