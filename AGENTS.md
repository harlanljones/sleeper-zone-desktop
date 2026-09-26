# sleeper-zone-desktop — Development Agent Guide

## Project intent

A web application cloned and rebuilt in Rust for performance. The Rust
implementation is the product; the original web app is reference material
only — behavior parity is measured, not guessed.

## Status

Milestone 4 complete: egui shell (`src/app/`) — first-run username →
league picker → live scoreboard rendering `TrackerSnapshot`s from the
tracker thread's watch channel; config + player-cache persisted under
`~/.config/sleeper-zone/` (`%APPDATA%\sleeper-zone\` on Windows).
Headless smoke: `cargo run -- --check-league
<league_id>`. Run the app: `cargo run --release`. All four milestones
of the build plan are complete, plus expanded stat lines and Windows
packaging (`packaging/windows/`, `.github/workflows/windows.yml`);
remaining work (notifications, code signing) is deferred by design.

## Where to work (planned layout)

| Path | Purpose |
|---|---|
| `src/` | Application code |
| `benches/` | Criterion benchmarks — the evidence that the rebuild is faster |
| `tests/` | Integration tests |

## Commands

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo bench
```

## Quality gate

`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
green before declaring any change done. Performance claims need a
`cargo bench` comparison against the recorded baseline, never intuition.

## Non-negotiable boundaries

- **Correctness before speed.** A benchmark win that changes behavior is a
  bug, not an optimization. Parity tests guard observable behavior.
- **No unsafe without justification.** Every `unsafe` block carries a
  comment stating the invariant it upholds and why safe code cannot.
- **Dependencies are audited.** New crates need a reason in the PR/commit
  body; prefer std and existing deps.
- **Secrets and benchmark artifacts are gitignored.** Never commit tokens,
  `.env*`, or `target/`.

## Code style

Rust, edition 2024 (adjust once Cargo.toml exists). Match the surrounding
code: `cargo fmt` is authoritative (the formatter hook enforces it on save),
clippy with `-D warnings` is the lint bar. Comments explain *why*. Error
handling uses `Result` with `thiserror`-style typed errors at boundaries,
no `unwrap()` outside tests and provably-infallible spots.

## Commits

Conventional Commits: `feat:`, `fix:`, `perf:`, `refactor:`, `chore:`.
`perf:` commits must cite the before/after benchmark numbers.

## Progress reporting

Report against outcomes: gate green; benchmark delta vs baseline; parity
test status.
