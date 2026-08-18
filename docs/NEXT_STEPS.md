# Next Steps

This list is derived from the migration roadmap and current parity state.

## Immediate Priority (Post-M2)

1. Start M3 metadata services with mocked I/O first.
- Create metadata module structure in src/metadata.
- Introduce HTTP client abstraction and wiremock-based tests.

2. Port discid flow from discid.c.
- Define deterministic mapping tests before implementation.
- Keep I/O boundaries injectable for integration tests.

Status update:
- Metadata module structure: done
- DiscID deterministic flow and tests: done

3. Port MusicBrainz lookup and mapping from musicbrainz.c.
- Introduce injectable HTTP client trait and response decoding layer.
- Add wiremock fixtures for release/disc lookup paths.

Status update:
- MusicBrainz lookup/mapping core: done
- Injectable HTTP trait + wiremock fixtures: done

4. Port cover art lookup/download handling from coverart.c.
- Add metadata-to-download selection logic tests first.

Status update:
- Cover art lookup/download core: done
- Injectable HTTP + wiremock tests: done

5. Port AccurateRip lookup and parse behavior from accurip.c.
- Add parser tests using deterministic fixture blobs.

Status update:
- AccurateRip lookup/parser core: done
- Injectable HTTP + deterministic fixture and wiremock tests: done

6. Integrate metadata flow orchestration in app path.
- Wire DiscID -> MusicBrainz -> Cover Art -> AccurateRip in deterministic order.
- Keep disable flags and fallback behavior parity.

Status update:
- Metadata orchestration module in app path: done
- Disable-flag and fallback flow tests: done

7. Start M4 audio output pipeline.
- Implement WAV output path first with integration tests.

Status update:
- WAV output path: done
- WAV end-to-end write/read integration test: done

8. Continue M4 audio output pipeline.
- Implement FLAC output path with deterministic integration tests.

Status update:
- FLAC output path: done
- FLAC end-to-end write/read integration test: done

9. Continue M4 audio output pipeline.
- Add per-track writer flow and output dispatch for WAV/FLAC.

Status update:
- App-level per-track output dispatch for WAV/FLAC: done
- Dispatch behavior tests (write paths + unsupported format error): done

10. Continue M4 audio output pipeline.
- Add metadata embedding in per-track writer flow where codec support exists.
- Keep unsupported codecs behind explicit deferred errors.

Status update:
- FLAC Vorbis-comment metadata embedding in per-track writer flow: done
- Tag propagation tests for album/track/disc fields: done

11. Continue M4 and app integration.
- Add app-path integration tests that connect CLI settings to metadata orchestration entrypoints.
- Expand metadata embedding coverage to additional codecs as they are implemented.

Status update:
- App-path integration tests from CLI parsing to metadata orchestration/output writer entrypoints: done
- CLI-driven output dispatch and FLAC tag propagation integration checks: done

12. Continue M4 and app integration.
- Expand metadata embedding coverage to additional codecs as they are implemented.
- Keep unsupported codecs behind explicit deferred errors until implemented.

13. Start M5 paranoia-mode ripping control path.
- Add CDDA paranoia state-machine module with deterministic transition tests.
- Freeze parity expectations for retry and abort behavior against /cyanrip/src/cyanrip_main.c.

Status update:
- Paranoia state-machine module and regression tests: done
- Upstream behavior mapping and transition document: done

14. Continue M5 reader abstraction for paranoia mode.
- Define frame-reader traits that can emit read success/error/media-changed events.
- Add image-backed reader fixture path that drives the state machine without physical hardware.

Status update:
- Frame-reader traits and runtime state-machine bridge: done
- Image-backed fake reader with fault-injection tests: done

15. Continue M6 physical backend and reliability.
- Wire Linux drive reads into the same state-machine events.
- Add induced read-failure and interruption tests to assert parity around retries/finalization.

## After M2

16. Continue parity matrix maintenance as each feature lands.
- Update statuses in [../PARITY_MATRIX.md](../PARITY_MATRIX.md).
- Record accepted differences in [PARITY_NOTES.md](PARITY_NOTES.md).

## Paranoia Mode Design Reference

- See [CDDA_PARANOIA_STATE_MACHINE.md](CDDA_PARANOIA_STATE_MACHINE.md) for states, events, and parity notes.

## Definition of Ready For Each Task

A task should only move to implementation when:
- acceptance behavior is described in tests,
- fixture inputs are available or created,
- expected outputs are deterministic and documented.
