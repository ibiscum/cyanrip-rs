# Miscellaneous Developer Notes

## Running info-only mode with real CD TOC read

To read the disc TOC from a physical drive and print the C-style info report:

```bash
cargo run --features "backend-libcdio-sys paranoia" -- -I -N -A -U -P 0
```

With an explicit device path:

```bash
cargo run --features "backend-libcdio-sys paranoia" -- -I -N -A -U -P 0 -d /dev/cdrom
```

| Switch | Effect |
|--------|--------|
| `--features "backend-libcdio-sys paranoia"` | Enables the libcdio native drive adapter and the cdda/paranoia feature chain — required to activate `read_drive_toc_tracks`, `read_drive_hwinfo`, and the libcdio-backed `-I` code path |
| `--` | Separates Cargo arguments from the binary's own arguments |
| `-I` | Info-only mode: read drive TOC and print disc/track layout, no ripping |
| `-N` | Disable MusicBrainz lookup (avoids a network call during info mode) |
| `-A` | Disable AccurateRip lookup (avoids a network call during info mode) |
| `-U` | Disable Cover Art DB lookup |
| `-P 0` | Set paranoia level to 0 (none) — no retry/verify overhead; info mode does not rip frames, but this silences the paranoia default |
| `-d /dev/cdrom` | Select device explicitly; without `-d`, libcdio picks the default drive and `System device:` shows `<default>` |
