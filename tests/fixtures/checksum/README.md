# Checksum Fixtures

`fun512_vectors.json` contains deterministic vectors generated from the reference C algorithm in `fun512.c`.

Structure:
- zero: SHA-512 digest bytes all zero.
- sequence_0_to_63: digest bytes set to 0..63.

Keys under each set are output index values (`idx`), matching C behavior where `idx` influences permutation.
