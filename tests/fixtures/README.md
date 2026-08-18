# Fixture Index

This directory stores deterministic sample outputs used for migration regression tests.

Subdirectories:
- cue/
- naming/
- log/
- checksum/
- musicbrainz/
- coverart/
- accurip/

## cue/
- basic_audio_two_tracks.cue
- mixed_audio_data.cue

## naming/
- cases.json

## log/
- valid.log
- mismatch.log
- trailing.log
- no_checksum.log

## checksum/
- fun512_vectors.json

## musicbrainz/
- discid_multi_release.json
- discid_single_release.json
- discid_no_releases.json

## coverart/
- front.bin
- back.bin

Used by:
- src/metadata/coverart.rs tests

## accurip/
- db_valid.bin
- db_truncated.bin
- db_html_error.bin

Used by:
- src/metadata/accurip.rs tests

Notes:
- These are sample fixtures collected during M0 for future snapshot/contract tests.
- Values are deterministic and suitable for CI.
