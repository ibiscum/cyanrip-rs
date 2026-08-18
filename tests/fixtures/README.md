# Fixture Index

This directory stores deterministic sample outputs used for migration regression tests.

Subdirectories:
- cue/
- naming/
- log/
- checksum/

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

Notes:
- These are sample fixtures collected during M0 for future snapshot/contract tests.
- Values are deterministic and suitable for CI.
