# `-d/--device` Option Flow

Last updated: 2026-08-26

This document explains what `--device` controls and how runtime chooses image vs physical-drive paths from it.

## Purpose

`--device` provides the input source path for ripping and drive-oriented operations.

The value can point to:
- A disc-image path (for example `.cue`, `.bin`, `.nrg`, `.toc`).
- A physical device path (for example `/dev/cdrom`).

## CLI Surface

Options:
- `-d`
- `--device`
- Alias: `--dev_path`

CLI parser mapping:
- `src/cli.rs` parses the option into `CliArgs.device`.
- `CliArgs::to_config` maps it into `settings.dev_path`.

## Source Selection Rules (Run mode)

Run-mode source selection is based on `settings.dev_path` suffix classification:

- `.bin`, `.cue`, `.nrg`, `.toc` -> image source.
- Any other path -> physical source.

If `--device` is not provided:
- On linux builds with `cdda` + `backend-libcdio-sys`: default source is physical.
- On other builds: default source is image.

Classification is performed by `open_dev_kind` using suffix checks.

## Physical Device Fallback

When a physical path is selected and no explicit `--device` value is present, physical reader paths use `/dev/cdrom` as fallback.

This fallback is used by:
- full-rip physical acquisition and TOC boundary resolution,
- find-offset scanning,
- other physical preflight calls that rely on drive reads.

## Mode-specific Behavior

### Info-only mode (`-I`)

- Linux + `cdda` + `backend-libcdio-sys` builds read hardware info and TOC using the configured device path.
- Other builds print an info report without physical TOC reads.

### Find-offset mode (`-f`)

- Linux + `cdda` + `backend-libcdio-sys` builds use the device path for drive probing, TOC read, and offset search.
- Other builds report that find-offset is unavailable in this build configuration.

### Full-rip run mode

- Device path controls image-vs-physical source selection.
- Physical source requires linux + `cdda`; additional TOC parity paths require `backend-libcdio-sys`.

## Practical Examples

Use a physical drive path:

```bash
cyanrip-rs -d /dev/cdrom -o flac
```

Use a CUE image source:

```bash
cyanrip-rs -d album.cue -o flac
```

Run with no explicit device (build-dependent defaults apply):

```bash
cyanrip-rs -o flac
```

## Regression Coverage

- CLI mapping in `src/cli.rs` (`golden_c_style_full_rip_invocation`) verifies that `-d /dev/cdrom` populates `settings.dev_path`.
