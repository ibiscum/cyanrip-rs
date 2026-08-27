# `-W/--no-deemphasis` Option Flow

Last updated: 2026-08-27

This document explains the purpose of `--no-deemphasis`, how it affects automatic deemphasis behavior, and how it interacts with related options.

## Purpose

`--no-deemphasis` disables automatic deemphasis.

Automatic deemphasis normally applies when deemphasis is enabled and track metadata indicates pre-emphasis. This flag turns that automatic path off.

## CLI Surface

Options:
- `-W`
- `--no-deemphasis`

Alias:
- `--no_deemphasis`

Flag type:
- boolean switch (disabled by default)

Examples:

```bash
cyanrip-rs -W -o flac -d disc.cue
```

```bash
cyanrip-rs --no-deemphasis -o wav,flac -d /dev/cdrom
```

## Parse and Settings Mapping

`--no-deemphasis` is parsed into `CliArgs.no_deemphasis` and mapped as:
- `settings.deemphasis = !no_deemphasis`

So when `-W/--no-deemphasis` is present, `settings.deemphasis` becomes `false`.

## Runtime Flow

Per track, processing path selection evaluates:
- HDCD path first,
- then deemphasis path,
- then passthrough.

Automatic deemphasis selection condition:
- `deemphasis == true` and track has pre-emphasis metadata.

With `--no-deemphasis` set:
- `deemphasis == false`, so automatic deemphasis is skipped.

## Interaction With Related Options

- `-E/--force-deemphasis`:
  - overrides auto-disable behavior,
  - still applies deemphasis even when `--no-deemphasis` is set.
- `-H/--hdcd`:
  - has higher precedence than deemphasis paths,
  - if HDCD is selected, deemphasis path is not used.

## Output Effects

When only `--no-deemphasis` changes behavior, output remains on the normal non-HDCD bit-depth path; the key difference is that pre-emphasis flagged tracks are not automatically deemphasized.

## Regression Coverage

Coverage includes:
- CLI mapping checks confirming `-W` flips `settings.deemphasis` to false.
- processing-path tests confirming automatic deemphasis is disabled when deemphasis is false.
- interaction tests confirming `-E` can still force deemphasis with `-W` set.
- scenario-runner checks validating `-W` output matches plain non-deemphasized output for pre-emphasis fixtures.
