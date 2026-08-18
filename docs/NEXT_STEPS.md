# Next Steps

This list is derived from the migration roadmap and current parity state.

## Immediate Priority (Post-M2)

1. Start M3 metadata services with mocked I/O first.
- Create metadata module structure in src/metadata.
- Introduce HTTP client abstraction and wiremock-based tests.

2. Port discid flow from discid.c.
- Define deterministic mapping tests before implementation.
- Keep I/O boundaries injectable for integration tests.

## After M2

3. Continue parity matrix maintenance as each feature lands.
- Update statuses in [../PARITY_MATRIX.md](../PARITY_MATRIX.md).
- Record accepted differences in [PARITY_NOTES.md](PARITY_NOTES.md).

## Definition of Ready For Each Task

A task should only move to implementation when:
- acceptance behavior is described in tests,
- fixture inputs are available or created,
- expected outputs are deterministic and documented.
