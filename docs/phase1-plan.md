# Muir — Phase 1 Plan (PR-by-PR)

**Status:** Iterating. PR 1 expanded; PR 2+ to be detailed as we work through them.
**Parent doc:** `design.md`

---

## Testing philosophy for this phase

Tests are continuous, not a phase. Every PR in this plan ships with its own tests as
an acceptance criterion. Three test scopes in play:

- **Unit tests** — `#[cfg(test)] mod tests` in the same file as the code. Default
  choice for type-level and function-level checks.
- **Integration tests** — files in `tests/` at the crate root. Used when we cross
  module boundaries or need a real fixture (e.g., wav file in Phase 1's final PR).
- **Doctests** — `///` examples in public item docs. Good for the `Detector` and
  `AudioDetector` traits once we have them, because the examples double as usage docs.

Benchmarks (Criterion) come in Phase 2, not here. Phase 1's performance should just
be "correct and not obviously pathological." We'll measure properly when we're on
aarch64.

## PR arc (outline)

**Sub-phase 1 — Scaffolding**
- **PR 1:** Project scaffold, licenses, CI, `cargo init`

**Sub-phase 2 — Core types + `Detector` supertrait**
- **PR 2:** Core types module (`Detection`, `Event`, `Modality`, `LatLon`, ...)
- **PR 3:** `Detector` supertrait + `DetectError`

**Sub-phase 3 — `AudioDetector` subtrait**
- **PR 4:** `AudioDetector` subtrait + test-only mock impl

**Sub-phase 4 — End-to-end wiring**
- **PR 5:** Config loading (`Config` struct, JSON via serde)
- **PR 6:** CLI scaffold (clap, smoke-test `cargo run`)
- **PR 7:** Audio ingestion (WAV read → 32 kHz mono → 5 s windows)
- **PR 8:** Model fetch script (`scripts/fetch_birdnet.sh`)
- **PR 9:** `BirdnetDetector` (wraps `birdnet_onnx::Classifier`, impl `AudioDetector`)
- **PR 10:** Sink trait + stdout JSON sink
- **PR 11:** `main.rs` wiring — ingest → inference → sink works end-to-end

**Sub-phase 5 — Fixture-backed integration test**
- **PR 12:** Sourced-fixture `tests/end_to_end.rs` — asserts known-species detection

---

## PR 1 — Project scaffold

```
Goal
    A buildable, linted, tested empty crate with license, CI, and the directory
    layout from design.md §7. Nothing in src/ yet except what cargo init gives you.

Files introduced
    .gitignore
    Cargo.toml
    LICENSE-APACHE
    README.md  (one-paragraph stub + model license caveat)
    rust-toolchain.toml
    .github/workflows/ci.yml
    src/main.rs  (cargo init default — prints something)

Dependencies added
    None yet. Resist the urge. Add deps in the PR that first needs them.

Acceptance criteria
    1. `cargo build` succeeds on your Mac.
    2. `cargo clippy -- -D warnings` succeeds.
    3. `cargo fmt --check` succeeds.
    4. `cargo test` succeeds (no tests yet, but the command must exit 0).
    5. CI runs all four of the above on push/PR and is green.
    6. .gitignore covers: target/, models/, *.onnx, *.tflite, .DS_Store, /muir.json
       (so local configs stay local).

Tests
    None yet — you have nothing to test. But CI must pass the empty `cargo test`.

What you'll learn
    - rust-toolchain.toml semantics (pinning channel + components like clippy, rustfmt)
    - The default cargo project layout and what each file does
    - GitHub Actions + the actions-rs or dtolnay/rust-toolchain setup patterns
    - Enough of Apache-2.0 to write a correct header (year, copyright holder)

Decisions you'll make on your own
    - Binary-only (`cargo init --bin`) or bin + lib split now?
      Design doc §7 says single crate starting out, so `cargo init` default is fine.
      If you reach for a library layout on day one, that's premature structure.
    - rust-toolchain.toml channel — stable, pinned minor (e.g. "1.86"), or "stable"?
      My read: pin to a specific minor for reproducibility, bump intentionally.
    - README tone — I'd write it aspirationally right now (what muir will be) but
      clearly labeled "pre-MVP, under active development."

Rough size
    ~150 lines across all files, most of which is license + CI config boilerplate.
    One-nighter, realistically half an evening.

```

## PR 2+ — Not yet detailed

We'll expand these one (or two) at a time after each PR ships and you've had
real contact with the code. Writing out all 12 PRs in detail now would be the
same mistake I already made with the `AudioDetector` code draft — pre-committing
to shapes we haven't earned yet.

Next to expand after PR 1 merges: PR 2 (core types).
