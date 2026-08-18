# Log Fixtures

These fixtures model output and verification states used by `cyanrip_verify_log` semantics.

Files:
- valid.log: checksum line matches the body with a FUN512 digest.
- mismatch.log: checksum line exists but does not match body digest.
- trailing.log: valid checksum line exists, but trailing data follows it.
- no_checksum.log: no `Log FUN512:` marker line.
- report_start_expected.txt: deterministic start-report rendering snapshot.
- report_finish_expected.txt: deterministic finish-report rendering snapshot.

Expected verification outcomes:
- valid.log -> VALID
- mismatch.log -> MISMATCH
- trailing.log -> TRAILING_DATA
- no_checksum.log -> NO_CHECKSUM
