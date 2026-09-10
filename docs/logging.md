# Diagnostic Logging

Last updated: 2026-08-31

This document explains the diagnostic logging facility in cyanrip-rs: what it covers, how it differs from the program's protocol/report output, and how to control verbosity.

## Purpose

cyanrip-rs uses the [`log`](https://docs.rs/log) facade with an [`env_logger`](https://docs.rs/env_logger) backend for diagnostic messages (warnings and errors about degraded/failed conditions during metadata lookup and ripping). This is separate from the program's normal user-facing output (rip summary report, progress lines, `-I`/`-J`/`-Y` mode reports), which continues to be printed directly via `println!` to preserve upstream `cyanrip` (C) output parity and pass differential/CLI integration tests.

## What goes through the logger

- MusicBrainz HTTP 503 retry attempts (backoff wait + attempt count) — [src/metadata/musicbrainz.rs](../src/metadata/musicbrainz.rs).
- Metadata-flow warnings collected during disc ID computation, MusicBrainz lookup, cover art lookup, and AccurateRip lookup (e.g. lookup failed/skipped) — surfaced immediately after the metadata phase completes, before track extraction starts.
- Paranoia reads that complete without fully converging (best-effort corrected frames used).
- AccurateRip mismatch that persists after exhausting retry attempts.
- Output filename collisions between two or more selected tracks.

All of the above are emitted via `log::warn!` or `log::error!` in [src/app.rs](../src/app.rs).

## What stays as direct console output

- The final full-rip summary/report text (track list, benchmarks, AccurateRip status line, written files).
- Per-track ripping/encoding progress lines (`Ripping...`, `Track N read attempt X of Y...`, `AccurateRip verified...`, encoding progress bar).
- `-I/--info`, `-J/--cue-only`, and `-Y/--verify-log` mode reports.
- CLI-level parse/config errors printed in [src/main.rs](../src/main.rs).

## Verbosity Control

The logger is initialized once at process start in `main()`:

```rust
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
```

- Default level is `warn`, so warnings and errors are visible without any extra configuration.
- Set the `RUST_LOG` environment variable to change verbosity, e.g.:

```bash
RUST_LOG=info cyanrip-rs -s 103 -B ~/rips
RUST_LOG=debug cyanrip-rs -s 103 -B ~/rips
RUST_LOG=off cyanrip-rs -s 103 -B ~/rips
```

`RUST_LOG` follows standard `env_logger` filter syntax (level names, per-module filters, comma-separated directives).

## Implementation Status

Complete. Dependencies added in [Cargo.toml](../Cargo.toml); logger initialization in [src/main.rs](../src/main.rs); diagnostic call sites converted in [src/app.rs](../src/app.rs) and [src/metadata/musicbrainz.rs](../src/metadata/musicbrainz.rs).
