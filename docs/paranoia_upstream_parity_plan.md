# Upstream Paranoia Loop Parity Plan

Last updated: 2026-08-28

## Goal

Prepare a concrete, implementation-ready plan to align Rust paranoia-enabled rip behavior with upstream cyanrip loop semantics.

Primary parity target:
- For paranoia-enabled runs, use one integrated frame loop where paranoia-produced frames are the exact frames consumed by checksum and encode logic.

## Feasibility

Yes, this is feasible.

Current blockers are architectural, not conceptual:
- current runtime path performs a precheck pass and then a separate direct-read pass,
- Linux backend currently models paranoia states via Rust heuristics over raw sector reads,
- upstream parity requires tighter callback and frame-failure semantics in the active rip loop.

## Current vs Target

Current behavior:
- physical and image paranoia-enabled paths run a paranoia pass before final PCM acquisition,
- final PCM is produced by a second read path,
- callback counters are synthesized from Rust-side events.

Target behavior:
- single active loop in paranoia-enabled mode,
- frame bytes from the paranoia reader are consumed directly by checksum and encode path,
- retry/repair and callback status accounting match upstream intent,
- no post-paranoia fallback reread path.

## Required Work Items

1. Unify data path for paranoia-enabled runs
- extend paranoia run result to carry selected output frames for the pass that finalizes,
- convert those frames directly to PCM for output pipeline,
- remove second read pass from paranoia-enabled branches.

2. Align failure semantics to upstream policy
- define frame-read failure behavior explicitly for parity mode,
- support upstream-compatible handling of unrecoverable frame failures,
- keep media-change and quit behavior as hard stop paths.

3. Strengthen callback/state parity
- map runtime statuses to upstream-equivalent callback counters,
- preserve retry-limit finalize and retry-ready transitions,
- ensure reported counters represent integrated read-loop activity.

4. Isolate non-parity path where needed
- keep paranoia level 0 direct-read path unchanged,
- gate any temporary behavior under explicit parity TODO notes until complete.

5. Differential and regression coverage
- add deterministic tests asserting there is no second read pass in paranoia-enabled mode,
- add cases for retry repair, retry-limit finalize, media-change abort, and interruption,
- add differential checks against upstream for selected damaged-read scenarios.

## Suggested Patch Series

1. Data plumbing patch
- return final paranoia frames from reader run result,
- add unit tests for selected pass frame capture.

2. App integration patch
- switch paranoia-enabled physical and image flows to consume returned frames,
- remove precheck-plus-reread pattern.

3. Callback and policy parity patch
- tighten counter semantics and failure handling,
- align logs and status reporting.

4. Verification and docs patch
- expand differential tests,
- update option-flow and parity docs once behavior lands.

## Acceptance Criteria

1. In paranoia-enabled runs, no second direct-read pass occurs after paranoia completion.
2. Output PCM in paranoia-enabled runs is derived from the paranoia loop frames.
3. Retry/repair transitions and retry-limit finalize behavior are covered by tests.
4. Media-change and quit behavior match declared parity policy.
5. Parity matrix row text for paranoia loop can be promoted without caveats about precheck-plus-reread.

## Out of Scope For This Slice

- full multi-codec parity outside current WAV/FLAC scope,
- unrelated metadata pipeline changes,
- broad log formatting parity outside paranoia counters.
