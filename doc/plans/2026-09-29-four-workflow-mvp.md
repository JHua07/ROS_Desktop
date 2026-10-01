# Four-workflow MVP execution plan

**Spec:** `doc/specs/2026-09-29-four-workflow-mvp.md`

1. Freeze IPC types/event semantics and file ownership in spec. Baseline git, frontend and Rust checks before editing.
2. Root adds shared process supervisor, platform stop boundary, Tauri commands/event registration, and typed frontend process API. Unit-test duplicate start, stop, exit/output ordering, and structured failures.
3. Graph worker adds overview component wired to existing graph/environment IPC, covering loading/error/empty/browser. No Rust graph query changes without root coordination.
4. Echo worker adds validated direct-argv command builder and panel for selected graph topic, stream/stop and bounded output.
5. Launch worker adds workspace-contained target discovery/validation, argument parsing and direct-argv builder, plus panel.
6. Bag worker adds topic/output validation and direct-argv builder, plus panel. No additional ROS package absent a demonstrated runtime need.
7. Root integrates panels and reviews each diff/tests, updates architecture/README/spec, runs full checks and live smoke/error cases where available, records omissions and git state.

## Verification record (2026-09-29)

Windows: frontend typecheck/build and dist, cargo fmt/test/check, and the explicit live daemon test pass (18 unit tests and one live test). A live Pixi-environment CLI smoke confirmed echo output and graceful stop, launch ready output and stop, and bag recording and stop with five messages in finalized metadata. Validation unit tests reject malformed inputs and existing bag targets. The smoke did not invoke Tauri IPC or interact with the GUI; ProcessManager duplicate-start, unexpected-exit, forced-stop and shutdown paths have not had dedicated live lifecycle tests, nor has Ubuntu runtime. These remain verification gaps rather than claimed passes.

## Regression verification addendum (2026-10-01)

A parallel re-verification confirmed frontend typecheck/build, cargo fmt/check, and the IPC contract (all commands/events aligned, no `pixi run` in GUI paths). One new launch test (`rejects_symlink_escape`, added with the MVP commit) failed on this machine because local security software silently degrades symlink creation into a regular file. The test now verifies the created link with `symlink_metadata` and skips degraded environments, matching the existing no-privilege protection; escape detection logic is unchanged. Final count: 20 passed, one live test ignored. The Tauri IPC/GUI end-to-end gap remains open.
