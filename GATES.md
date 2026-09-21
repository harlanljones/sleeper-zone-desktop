# Gates: milestone-4 — egui shell

OWNS: src/app/**, src/main.rs, Cargo.toml, Cargo.lock, GATES.md

Scope: The desktop app: first-run username → league picker → live
scoreboard rendering TrackerSnapshots, config + player-cache
persistence under the XDG config dir, and a headless `--check-league`
mode that runs one real tracker tick for gate verification.

- [x] G1: Formatting is clean across the workspace.
  CHECK: cargo fmt --all -- --check && echo 'formatting gate passed'
  EXPECT: formatting gate passed
  EVIDENCE: automatic-evidence=v1; definition-sha256=ecec13ac4d1c1e14249073fb043aefea77cb0114aba7e0405307742da5760d18; exit=0; EXPECT=matched; output-sha256=10c622b3320455e6e252f4fab80c8cdb4a81f12c477fab8002e828243a89576d; output-bytes=23; shell=/bin/sh; cwd=/home/harlan/sleeper-zone-desktop; path=bc85cc4d4c0d/86 entries

- [x] G2: Clippy reports zero warnings with warnings denied.
  CHECK: cargo clippy --all-targets -- -D warnings && echo 'clippy gate passed'
  EXPECT: clippy gate passed
  EVIDENCE: automatic-evidence=v1; definition-sha256=ce898b7f4330c76fae9a8055f80f9db20a97217fc1fa902b0ff60af2fbbeb041; exit=0; EXPECT=matched; output-sha256=eab0f567016f7df809f0e1932a5bed731cd659a5b4d26cd15c292d4824e51209; output-bytes=92; shell=/bin/sh; cwd=/home/harlan/sleeper-zone-desktop; path=bc85cc4d4c0d/86 entries

- [x] G3: Full test suite is green, including config round-trip and
  path-resolution tests for the persistence layer.
  CHECK: cargo test && echo 'full test suite passed'
  EXPECT: full test suite passed
  EVIDENCE: automatic-evidence=v1; definition-sha256=01cc2d876697eae8a854520e6af1e5629a5736a61605896cc1661742ca0bdfd5; exit=0; EXPECT=matched; output-sha256=4beda511f738c843d7e21a4c50c99fb3cb0ea3c2bf01a4fbe1b745c0f84b6e9b; output-bytes=3816; shell=/bin/sh; cwd=/home/harlan/sleeper-zone-desktop; path=bc85cc4d4c0d/86 entries

- [x] G4: Headless live check: the real binary runs one tracker tick
  against production APIs for a known league id and prints a summary.
  CHECK: cargo run --quiet -- --check-league 289646328504385536 2>&1 | grep -q 'check ok' && echo 'headless check passed'
  EXPECT: headless check passed
  EVIDENCE: automatic-evidence=v1; definition-sha256=b90de2284d76a0b302c1661af9bd930c7af0ee0e8bf52dddabf0973e47fdc0b0; exit=0; EXPECT=matched; output-sha256=7cdbf89ced87e2172cce46054fbab776dbe3a3a7c57a0cbe8b3168492bdc2534; output-bytes=22; shell=/bin/sh; cwd=/home/harlan/sleeper-zone-desktop; path=bc85cc4d4c0d/86 entries
