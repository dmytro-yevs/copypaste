# ADR-0026: Clean build caches at Git commit and push boundaries

Status: accepted; replaces the previous compilation-unit budget cleaner.

## Decision

The `post-commit` and `pre-push` hooks invoke native `cargo clean` and
`flutter clean` commands. The custom compilation-unit tracking and eviction
implementation and the older partial-clean shell script are removed.

Cargo cleanup covers existing checkout-local workspace, module, and bridge
target directories. Flutter cleanup removes its build directory and
`.dart_tool`, including native-hook Rust builds. External Cargo target
directories and other worktrees are not cleanup targets.

Hooks check build processes and Cargo locks before cleaning and again before
each command. Active Rust, Flutter, Xcode, and Gradle builds cause cleanup to
be skipped. An idle Gradle daemon does not block cleanup. Process-inspection
failure also skips cleanup. These checks do not coordinate a build started
concurrently after the check.

Cleanup uses shared hooks on macOS, Windows Git Bash, and Android development
hosts. Python 3, Cargo, and Flutter must be available on the host. Missing tools
and cleanup failures are reported but do not fail commit or push. A successful
cleanup discards all local compilation caches, so the next build is slower.

Enable the hooks in each clone with `git config core.hooksPath .githooks`.
Hooks do not run for builds without a commit or push, so they cannot enforce a
continuous disk-space ceiling. `git commit --no-verify` still runs post-commit;
`git push --no-verify` bypasses pre-push.
