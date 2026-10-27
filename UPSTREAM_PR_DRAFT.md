# crash reports: a caught kernel panic on a Rayon worker is not an immediate crash (#7402)

## Change

A panic inside parallel kernel or meshing work runs on a Rayon worker. That thread does not inherit the command scope or `solidcraft_kernel::guarded()`, so the crash hook wrote an unrecovered `panic` report at once, with no design and no command. Rayon then resumes the panic on the calling thread, where `guard` turns it into `KernelError::Internal` and the hook does not run again. The operation failed cleanly, but the app still recorded a crash. Reports #6148, #2319 and #1337 are this shape (`Thread: (unnamed)` in the mesher).

While a command is running, a panic on a pool thread is now held and written when that command ends, with the command and the design, the same way a panic on the command's own thread is. It is marked `kernel_panic_recovered` when the worker itself was inside `guard`, or when the command failed with a kernel internal error (the guard on the calling thread caught the resumed panic). A pool panic while no command is open is still written at once.

The open-command count and the held panics share one lock, so a worker cannot queue a panic after the command has already drained the queue. This does not change B-rep, fillet, boolean or solidcore behaviour.

Fixes #7402

Changes a default: no

## Validation

`cargo test -p solidcraft-engine --lib crash::tests::` — 24 passed, 0 failed. The new cases:

- `a_worker_panic_during_a_command_is_recovered_with_the_command`: one report, kind `kernel_panic_recovered`, thread `solidcraft-pool-0`, base is the design from when the command started, replay is `diagnostics.crash_test` mode `worker`. Not an immediate unrecovered `panic`.
- `a_worker_panic_outside_the_kernel_guard_is_a_command_panic`: the same worker panic outside `guard` is one `command_panic` with that command and design, not `kernel_panic_recovered`.
- `a_worker_panic_outside_a_command_is_reported_at_once`: kind `panic`, no command, thread `solidcraft-pool-0`, journal base `new`.

Existing command, kernel and out-of-command panic tests in the same module still pass.

`cargo clippy -p solidcraft-kernel -p solidcraft-engine --all-targets -- -D warnings` — clean.
`cargo check -p solidcraft-kernel -p solidcraft-engine --target wasm32-unknown-unknown` — finished. The three wasm warnings are existing dead code in the engine, not these files.

## Screenshots of the behavior (when observable)

No viewport change. The bug is which crash report is written. The tests above read the report file (kind, thread, command, design, replay).

## Integration status

Branch is based on tangency-labs/solidcraft `main` at `3bea744823bf0a8ff48a8f165bdcd4f73af6ba95`. No upstream pull request opened. Not checked for later `main` conflicts after that commit.
