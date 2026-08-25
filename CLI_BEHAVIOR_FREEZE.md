# CLI Behavior Freeze (M0)

Last updated: 2026-08-20

This document freezes CLI expectations for defaults, validation behavior, and error messages.

Scope:
- Applies to argument parsing and pre-run validation only.
- Applies to behavior implemented in [src/cli.rs](src/cli.rs) and [src/lib.rs](src/lib.rs).
- Runtime ripping, metadata fetch, and encoder runtime failures are out of scope here.

## 1. Default Expectations

Expected defaults when invoked with no flags:
- action: Run
- dev_path: None
- offset: 0
- over_under_read_frames: 0
- max_retries: 10
- ripping_retries: 0
- speed: 0
- bitrate_kbps: 256.0
- outputs: [flac]
- sanitize_method: unicode
- paranoia_level: 3
- overread_leadinout: false
- decode_hdcd: false
- deemphasis: true
- force_deemphasis: false
- enable_replaygain: true
- disable_mb: false
- disable_accurip: false
- disable_coverart_db: false
- disable_coverart_embedding: false
- print_info_only: false
- generate_cue_only: false
- eject_on_success_rip: false
- find_drive_offset: false
- rip_indices_count: -1
- rip_indices: []
- release: None
- discnumber: 0
- totaldiscs: 0
- verify_log: None
- album_metadata: None
- track_metadata: []
- cover_specs: []

## 2. Validation Expectations

Validation behavior is frozen as follows:

- paranoia:
- accepted keywords: none, max
- numeric parsing follows C strtol-like semantics:
- bogus -> 0
- 2abc -> 2
- out-of-range values return error

- cover size:
- accepted values: -1, 250, 500, 1200
- any other value returns error

- sanitize method:
- accepted values: simple, os_simple, unicode, os_unicode
- any other value returns error

- outputs:
- comma-separated values accepted
- duplicates return error
- unknown output returns error
- outputs=help switches action to ShowOutputsHelp and short-circuits late validations

- tracks:
- comma-separated integer list
- duplicates return error
- normalized order is sorted ascending

- pregap:
- accepted format: N=default|drop|merge|track
- N must be in [1, 197]
- invalid index or action returns error

- mode conflicts:
- info + cue-only is invalid and returns error
- find-offset + info is invalid and returns error
- find-offset + cue-only is invalid and returns error

- release:
- positive integer maps to index selection
- non-integer token maps to id selection
- zero or negative integer returns error

- disc:
- accepted format: disc/total or disc
- disc must be > 0
- total must be > 0 when present
- disc cannot be greater than total

## 3. Special-Flow Expectations

Precedence is frozen:
1. verify-log mode (-Y/--verify-log) short-circuits everything else and returns action VerifyLog.
2. outputs help mode (-o help) short-circuits late validations and returns action ShowOutputsHelp.
3. otherwise normal Run validation path is applied.

Side-effect behavior is frozen:
- info-only (-I):
- eject_on_success_rip = false

- cue-only (-J):
- disable_accurip = true
- disable_coverart_db = true

- find-offset (-f):
- disable_accurip = false
- disable_mb = true
- disable_coverart_db = true
- offset = 0
- over_under_read_frames = 0
- eject_on_success_rip = false

## 4. Error Message Freeze

The following validation error messages are treated as stable contract strings:
- Directory name scheme must contain {format} with multiple output formats!
- -J (only generate a CUE sheet) cannot be used with -I (only print info)!
- -f (find drive offset) cannot be used with -I (only print info)!
- -f (find drive offset) cannot be used with -J (only generate a CUE sheet)!
- Invalid max coverart size <n> (must be 250, 500, 1200 or -1)
- Invalid sanitation method <value>
- Invalid track idx for pregap: <n>
- Invalid pregap action <value>
- Invalid release index <n>!
- Invalid discnumber <n>
- Invalid totaldiscs <n>
- discnumber <n> is larger than totaldiscs <n>
- Invalid format "<value>"
- Duplicated format "<value>"
- Duplicated rip idx <n>
- Invalid paranoia level <n> must be between 0 and 3

Notes:
- Clap-generated parse/usage formatting for malformed flag syntax is not frozen byte-for-byte.
- Custom validation error strings above are frozen and covered by tests.

## 5. Test Lock

Behavior is locked by tests in:
- [src/cli.rs](src/cli.rs)
- [src/lib.rs](src/lib.rs)

Any future change to defaults, validation semantics, short-circuit order, or the frozen custom error strings requires:
1. updating this document,
2. updating tests,
3. recording rationale in migration notes.
