# Muir — Design Doc

**Status:** Living document. Edit freely.
**License:** Apache-2.0
**Last updated:** 2026-04-23

---

## How to read this doc

Sections are tagged:

- `[DECIDED]` — we've agreed, no more debate unless new info shows up.
- `[LEAN]` — current recommendation, open to revision.
- `[OPEN]` — real decision to make, with options laid out. This is where your input matters most.
- `[RESEARCH]` — unknowns that need investigation before we can decide.
- `[DEFERRED]` — decision belongs to a later phase; record current thinking, revisit then.

---

## 1. Problem statement

Conservation field sites (national parks, reserves, research stations, anti-poaching zones)
are drowning in sensor data — camera traps, acoustic recorders, PIR triggers — but most
deployed tooling is siloed: a camera-trap platform that doesn't know about audio, an acoustic
platform that doesn't know about cameras, no shared temporal event model, and uplink designs
that assume broadband rather than the LoRa / satellite / intermittent-cellular reality of
actual field deployments.

Muir is a Rust-based edge gateway that ingests heterogeneous sensor streams, routes them to
specialist inference models (BirdNET for acoustic species ID first, MegaDetector for camera
traps later, gunshot/chainsaw detectors as plugins), fuses outputs into higher-confidence
events with temporal correlation, persists locally, and syncs to upstream systems over
whatever connectivity is available. A local small-LLM layer handles summarization and
natural-language query over the event store for researchers and rangers.

**[DECIDED]** Problem statement signed off.

## 2. Non-goals

Explicitly **not** in scope:

- **Not** a general-purpose LLM serving framework. LLMs in Muir exist only for summarization
  and reasoning over structured events, never in the inference hot path.
- **Not** a model training platform. Muir consumes pre-trained models from upstream projects
  (BirdNET-Analyzer, MegaDetector, etc.) and does not retrain them.
- **Not** a replacement for SMART, Wildlife Insights, or Arbimon. Muir aims to *feed* these
  systems, not compete with them. Interop is a feature.
- **Not** a cloud service. The gateway runs at the edge. Any cloud component is optional and
  user-operated.
- **Not** a mobile app. There may eventually be a web dashboard served by the gateway, but
  no iOS/Android clients.

**[DECIDED]** Revisit only when scope needs to expand.

## 3. Target users

- **Primary (MVP):** Field research biologists deploying sensors at a study site, running
  their own hardware, self-hosting the gateway.
- **Secondary:** Conservation NGO technical staff (WildLabs-adjacent), managing multiple
  sites.
- **Tertiary:** Government-operated anti-poaching units (future, once hardened).

**[DECIDED]** Biologist-first for the MVP. But — see §4, *Design principles* — the
architecture is designed from day one to generalize toward NGO and anti-poaching use cases
without rewrites. The MVP narrows the *users* we ship for; it does not narrow the *shape* of
the system we build.

## 4. Design principles

The tension: Muir should eventually be a one-stop-shop edge gateway for varied conservation
efforts, but we also need a working MVP on a reasonable timeline while PDaddy learns Rust.
These principles let us hold both goals at once.

1. **Modality-agnostic core, specialist edges.** The event model, fusion engine, and sink
   layer know nothing about birds or cameras specifically. Modality-specific logic lives
   only in ingest and inference modules. Adding a new modality later means writing a new
   inference adapter, not rewiring the core.

2. **Plugin-ready before plugins exist.** Even though Phase 1 compiles BirdNET in directly,
   inference adapters conform to a `Detector` trait from day one. When Phase 5 introduces
   wasmtime plugins, external detectors implement the same trait. No retrofit.

3. **Versioned public contracts.** The `Detection` and `Event` structs carry a schema
   version field and are serialized via serde. Breaking changes are a visible, deliberate
   act, not an accident.

4. **Performance discipline without premature optimization.** Measure before optimizing.
   But — benchmarks in CI so we *know* when we regress. Allocations in the audio hot path
   are a code-review concern from day one, because refactoring allocation patterns later is
   miserable.

5. **Dependency hygiene.** Every new crate is justified in the PR that adds it. Avoid
   abandoned or single-maintainer deps where plausible. This is one of the interview signals
   for a serious Rust engineer.

6. **Test seriously.** Real audio fixtures (a 10-second wav with known bird calls),
   integration tests that end-to-end the pipeline, unit tests on schema and fusion logic.
   BirdNET produces probabilistic output; a fixture harness is our only defense against
   silent regressions.

7. **Idiomatic Rust, not defensive Rust.** Use the ownership model. Don't smear
   `Arc<Mutex<_>>` everywhere because async "feels scary." When in doubt, prefer channels
   over shared state.

8. **Sensitive-data minimization.** Sensor locations, device identifiers, and other
   metadata that could be used to harm study subjects, operators, or endangered species
   are treated as privileged. External outputs (MQTT payloads, log files, uplinked
   summaries) redact precise location data by default — typically coarsened to a grid
   cell or site ID — while local storage retains full fidelity behind the gateway's
   auth boundary. Particularly relevant for anti-poaching and endangered-species
   contexts. Redaction is a config-driven sink-layer concern, not a schema concern; the
   canonical `Detection` and `Event` carry full data.

## 5. System overview

```
      ┌─────────────────────────────────────────────────────────────┐
      │                        muir gateway                         │
      │                                                             │
      │  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐  │
sensors│  │  ingest  │──▶│inference │──▶│  fusion  │──▶│   sink   │  │──▶ MQTT / HTTP
──────▶│  │          │   │          │   │          │   │          │  │──▶ SQLite (local)
      │  └──────────┘   └──────────┘   └──────────┘   └──────────┘  │
      │       ▲              ▲              │              │        │
      │       │              │              ▼              ▼        │
      │       │         ┌──────────┐   ┌──────────┐   ┌──────────┐  │
      │       │         │  models  │   │  event   │   │ llm      │  │──▶ summaries
      │       │         │  (ONNX)  │   │  store   │   │ (later)  │  │
      │       │         └──────────┘   └──────────┘   └──────────┘  │
      │       │                                                     │
      │  ┌────┴──────────────────────────────────────────────────┐  │
      │  │                     config + plugins                  │  │
      │  └───────────────────────────────────────────────────────┘  │
      └─────────────────────────────────────────────────────────────┘
```

Data flow: sensors push bytes → ingest decodes/windows → inference produces raw detections
→ fusion correlates across streams/time into events → sink persists and publishes.

## 6. Design decisions

### 6.1 Async runtime

**[DECIDED] Tokio.** No real alternative for this domain. Ecosystem coverage (MQTT, HTTP,
DB drivers) is overwhelming. `async-std` is effectively dead. `smol` is lovely but thinly
supported for our deps.

### 6.2 BirdNET inference

**[DECIDED] Use the `birdnet-onnx` crate (v2.x, MIT-licensed) as the BirdNET inference
adapter**, accessed through our detector trait (shape still being designed in §6.11).
The crate is built on `ort`
(which is exactly what we'd have chosen for direct integration), supports BirdNET v2.4
and v3.0, Perch v2, and BSG Finland out of the box, handles the lat/lon/day-of-year
"range filter" meta model cleanly (see §6.4), offers batch inference and GPU paths, and
statically links ONNX Runtime by default so we avoid DLL-version pain at deploy time.

**[DECIDED] MVP model: BirdNET v3.0.** Per PDaddy, we start on the newer architecture
(32 kHz / 5 s windows, 1280-dim embeddings) and benchmark against v2.4 in a later phase.
v3.0's embeddings unlock interesting downstream options — similarity search over past
detections, lightweight clustering for "unknown caller" events — that v2.4 can't provide.
The tradeoff is fewer battle-hardened deployments than v2.4; we'll validate against
fixtures carefully in Phase 1.

**[DECIDED — Phase 2+ goal] Lighter/faster model for edge performance.** Once Phase 1
ships a correct end-to-end pipeline on v3.0, we explicitly pivot to optimizing model
footprint for edge deployment (Oracle ARM VM, eventually Raspberry Pi). Candidates,
roughly in order of how easy the swap is:

1. **BirdNET v2.4** — same model family, smaller (144,000 samples vs. 160,000),
   faster inference, more mature. The `birdnet-onnx` crate handles it natively;
   swap is one config line. This is the default fallback for lighter inference.
2. **Quantized v3.0 (INT8)** — roughly 4× smaller, roughly 2–3× faster on CPU, minor
   accuracy loss. Requires either an upstream quantized artifact or we do the
   quantization ourselves. Check whether `birdnet-onnx` or ONNX Runtime gives us
   this for free before doing it by hand.
3. **Perch (lighter variant)** — Google Research's model; commercial-friendly
   license. Investigate whether a "Perch Lite" or similar small variant exists.

Phase 2 includes a benchmarking harness (using Criterion, per §4 principle 4) that
measures per-segment inference latency and memory on aarch64 for each candidate.
Decision on the chosen lighter model happens after the numbers land, not before.

**What Muir implements:**

- The `Detector` trait (modality-agnostic, §4 principle 2).
- A `BirdnetDetector` adapter that holds `birdnet_onnx::Classifier` + optional
  `RangeFilter`, and translates the crate's `Prediction` into our `Detection` schema
  (§6.6). This is the layer that keeps the crate a swappable implementation detail rather
  than a hard coupling.
- Audio preprocessing in `ingest::audio_file` — decode `.wav`, resample to **32 kHz mono**
  for BirdNET v3.0 (the MVP model), chunk into non-overlapping **5-second segments**
  (160,000 samples). The crate consumes raw `f32` samples at the expected rate; we own
  the decode path. When we add v2.4 benchmarking later, the adapter queries its own
  `sample_rate()` / `segment_samples()` — see §6.11.
- `scripts/fetch_birdnet.sh` — download model + labels from upstream, pin versions, hash-check.

**What the crate handles for us:**

- ONNX Runtime linkage (static by default; no runtime DLL surprises).
- Model loading, tensor shape handling, model-type auto-detection.
- Inference (single + batch, CPU + CUDA).
- Top-K filtering and confidence thresholding.
- Label-file parsing.
- The BirdNET "range filter" meta model — separate from the main classifier, called once
  per location+date rather than per audio frame.
- `CancellationToken` and per-inference timeout (integrates with tokio-style graceful
  shutdown).

**Strategic note — Perch as a commercial-friendly path.** The crate also supports
**Perch v2** (Google Research bioacoustics classifier). Perch is typically Apache-2.0
licensed (verify at selection time), which would make it commercial-friendly — unlike
BirdNET's CC-BY-NC-SA restriction documented in §6.3. Consequence: when we reach for
tertiary-tier users (anti-poaching units operating under commercial licensing
requirements), swapping the underlying model is a days-not-weeks change because the crate
already speaks Perch. BirdNET stays primary for the MVP given its species coverage and
scientific credibility; Perch is a known-good escape hatch.

### 6.3 Model and audio licensing

**[DECIDED]** The BirdNET model weights are **CC BY-NC-SA 4.0**. Our Apache-2.0 code is
compatible *alongside* the model, but the model is not commercially redistributable and
derivative models inherit the license.

Operational implications for Muir:

- We do **not** bundle the model in the released binary or container.
- `scripts/fetch_birdnet.sh` downloads from the official source on first run.
- `README.md` states the model license explicitly and the non-commercial restriction.
- If we ever support commercial users, we need a commercial-friendly acoustic classifier
  path (retraining, a differently-licensed model, etc.). That's a Phase 6+ concern.

### 6.4 Audio pipeline

BirdNET v3.0 (our MVP target) is trained on **5-second windows of 32 kHz mono PCM** and
outputs 1280-dimensional embeddings alongside species predictions. For reference, v2.4
uses **3-second windows at 48 kHz**. Both versions use **latitude, longitude, and
week-of-year** as auxiliary features (the model uses location and season to prior the
species distribution). This means the gateway must know its geographic location —
location is a first-class config concern, not an afterthought.

Implementation note: `birdnet-onnx` exposes the location+date priors as a separate
`RangeFilter` meta model rather than folding them into the main classifier. We call the
main classifier per audio segment and the range filter at most once per location+date,
then post-filter / re-rank predictions. That's cleaner than baking priors into the inner
loop and is what our `BirdnetDetector` adapter will do.

**[DECIDED] Non-overlapping windows at the model's native length** for Phase 1 —
5 seconds for v3.0, 3 seconds for v2.4. Matches the reference implementation. Revisit
overlap if recall is poor against fixtures.

**[DECIDED] Audio source for Phase 1:** read a `.wav` file from disk, process end-to-end.
Microphone input (via `cpal`) is deferred to Phase 2 as a stretch goal.

### 6.5 Configuration format

Options considered:

1. **TOML** — idiomatic Rust, supports comments, Cargo uses it.
2. **JSON** — most universally known, unanimous across research/NGO tooling, first-class
   serde support. No comments without extensions (JSON5/JSONC).
3. **YAML** — common in ML world, but hand-editing pitfalls (indentation, type coercion).

**[DECIDED] JSON for MVP.** Researchers are overwhelmingly JSON-literate,
TOML is Rust-native but less universal, and JSON is the unanimous option across target
users. The no-comments tradeoff is acknowledged — we'll mitigate with a well-documented
schema file (`docs/config-schema.json`) and clear examples in `README.md`.

**[DEFERRED] Multi-format support.** Because we define config as a serde-derived struct,
adding TOML or YAML support later is roughly five lines of code per format — different
parser, same struct. We start with JSON only; if real users request TOML/YAML we add them
without touching the config model.

Quick clarification on tooling (this comes up because PDaddy mentioned clap): `clap` parses
**CLI arguments** (e.g., `muir --config ./muir.json --verbose`), not the config file itself.
The config *file* is parsed by `serde_json`. Typical pattern is:

```rust
#[derive(clap::Parser)]
struct Args { #[arg(long)] config: PathBuf, #[arg(long)] verbose: bool }

#[derive(serde::Deserialize)]
struct Config { lat: f64, lon: f64, sensor_id: String, /* ... */ }

let args = Args::parse();
let config: Config = serde_json::from_reader(File::open(&args.config)?)?;
```

Two small things, both idiomatic.

### 6.6 Event schema

The event is the public contract of the system. Everything downstream (fusion, sink, LLM,
dashboard) keys off it.

Draft:

```rust
struct Detection {
    id: Uuid,                               // unique per detection
    schema_version: u16,                    // §4 principle 3
    source: SensorId,                       // which sensor produced this
    sensor_location: Option<LatLon>,        // privileged; see note below + §4 principle 8
    modality: Modality,                     // Audio | Camera | Pir | ...
    model: ModelId,                         // "birdnet-v3.0" etc.
    model_version: String,
    timestamp: DateTime<Utc>,               // when the underlying event occurred
    ingested_at: DateTime<Utc>,             // when Muir saw it
    label: String,                          // species / object class
    confidence: f32,                        // 0.0..1.0
    bbox: Option<BBox>,                     // for vision models
    raw_meta: serde_json::Value,            // model-specific extras
}

struct Event {                      // produced by fusion
    id: Uuid,
    schema_version: u16,
    detections: Vec<DetectionRef>,  // the raw detections that support this event
    summary: String,                // "human+vehicle in grid C4 at 14:23"
    severity: Severity,             // Info | Notable | Alert
    window: TimeRange,
    location: Option<LatLon>,
}
```

**[DECIDED] Alert handling.** Severity lives on `Event` (`Severity::Alert`). We do not
introduce a separate `Alert` type. Alerting (paging a ranger, etc.) is a sink-layer concern
that reads `severity` and decides whether to emit a notification. De-duplication of
repeated high-severity events is a sink-layer problem when we implement notifications in
Phase 4, not a modeling problem now.

**[DECIDED] Time representation.** UTC everywhere internal. Zoned only at presentation
(logs, UI). `timestamp` and `ingested_at` are both `DateTime<Utc>`.

**Detection generality.** Is `Detection` bird-specific or generic across
modalities? Is a `String` label enough?

Design answer:

- **`Detection` is modality-agnostic by design.** The `modality` and `model` fields
  disambiguate what produced it; every downstream consumer (fusion, sink, LLM) treats
  `Detection` uniformly regardless of whether the source was audio or camera. Adding a new
  modality does not touch the struct.
- **`label: String` is the canonical human/machine-readable class** — e.g., `"Cyanocitta
  cristata"` for BirdNET, `"human"` or `"vehicle"` for MegaDetector. Single-string label
  keeps the schema flat and serializable.
- **`raw_meta: serde_json::Value` is the extensibility hatch** for model-specific richness
  (BirdNET's per-species confidence distribution, MegaDetector's detection array, etc.).
  This is explicitly unstructured, and downstream code must not rely on its shape; it's for
  archival, debug, and opt-in advanced features.
- **If we ever need structured labels** (taxonomy tree, common name, scientific name, IUCN
  status, ontology URI), we evolve by adding optional fields — `taxon: Option<TaxonRef>` —
  and bump `schema_version`. Existing consumers keep working.

**[DECIDED]** Keep the schema as drafted. Reconsider if Phase 3's camera integration or
Phase 5's plugins surface a field the `Detection` can't carry cleanly.

**On `sensor_location` and privacy.** Location stays on `Detection` because the LLM
summarization layer (Phase 5) genuinely benefits from it — *"chainsaw detected near grid
C4 at 14:23"* is more useful than *"chainsaw detected"*. Exposing precise lat/lon in
every outbound event payload is a real attack surface, especially for anti-poaching
deployments where sensor-location leakage defeats the purpose. Resolution (per §4 principle 8): the canonical `Detection` holds full-fidelity
location; the **sink layer** is responsible for redaction on externally-bound outputs.
Redaction modes (config-driven, defaults to `CoarsenedGrid`):

- `Full` — exact lat/lon. Local-only by default; off-box only for trusted sinks.
- `CoarsenedGrid` — round to a configurable grid (e.g., 1 km). Safe default for most.
- `SiteIdOnly` — replace with an opaque site identifier. Safe default for anti-poaching.
- `None` — strip location entirely.

Implementation lives in the sink, not the model. Phase 4 concern; naming it now so the
sink's responsibility is explicit.

### 6.7 Persistence

**[DECIDED] SQLite via `rusqlite`** for the event log. `sqlx` has compile-time query
checking but is heavier and has a build-time DB requirement; for a single-file embedded DB
`rusqlite` is the idiomatic choice. Phase 4 item — we defer implementation, not the
decision.

### 6.8 Uplink

**[DECIDED] MQTT via `rumqttc`.** Universal in IoT, works over unreliable links, supports
QoS 1 for store-and-forward semantics naturally. Phase 4 item.

### 6.9 LLM layer

**[DECIDED — provisional] `llama-cpp-2` bindings** with a Q4-quantized Phi-3-mini or
Gemma-2-2B. Phase 5 item. We re-pick the specific model based on what's current and
well-supported when we get there; the bindings decision holds unless the Rust LLM
ecosystem shifts.

### 6.10 Plugin model

**[DEFERRED — leaning wasmtime]** Phase 6 item. Revisit when we actually have two or more
real detectors worth making plug-able. For now, the §4 principle (*plugin-ready before
plugins exist*) is honored by requiring all Phase 1 detectors to implement whichever
detector trait shape we converge on in §6.11.

### 6.11 Detector trait (open co-design)

**Purpose.** The detector trait is the abstraction layer that makes Muir's pipeline
agnostic to which concrete model or runtime produces detections. Adding a new detector
— `birdnet-onnx` today, a MegaDetector adapter in Phase 3, a wasmtime plugin in
Phase 6 — should mean writing one new `impl` and zero changes anywhere else in the
gateway. The trait exists from commit #1 even though we'll only have one implementor
for months. This is how we honor §4 principle 2 (*plugin-ready before plugins exist*).

**The question: one trait for all modalities, or per-modality traits?**

§4 principle 1 (modality-agnostic core) is satisfied on the *output* side — every
detector emits `Detection`s in the same schema regardless of source. The open question
is whether the *input* side can also be uniform.

The honest tension is that different modalities have genuinely different natural input
types:

- Audio detectors want `&[f32]` samples at a known sample rate.
- Vision detectors want `ImageFrame { pixels, width, height, pixel_format }`.
- Passive detectors (PIR, seismic, door sensors) want just a trigger event with
  metadata — no signal payload at all.

Rust's type system forces us to pick how to express that in the trait.

**Three realistic options.**

**Option A — Per-modality traits.** `AudioDetector`, `VisionDetector`,
`PassiveDetector`, each with its own strongly-typed `detect()` method. Compile-time
safety: you cannot accidentally pipe an image frame into BirdNET — it won't type-check.
Cost: no unified `Vec<Box<dyn Detector>>` registry; each modality's pipeline stage
holds only the trait objects it can actually feed.

**Option B — Generic supertrait + per-modality subtraits.** One common `Detector`
supertrait holding shared metadata (`id`, `version`, `modality`, etc.). Modality-
specific `detect()` lives in subtraits:

- `AudioDetector: Detector` with `async fn detect(&self, samples: &[f32]) -> …`
- `VisionDetector: Detector` with `async fn detect(&self, frame: &ImageFrame) -> …`

You get conceptual unity — there is one thing called "a detector," and audio/vision
variants share identity and metadata — while the inference call itself stays
strongly typed. Probably the cleanest middle ground.

**Option C — Single trait with an `Input` enum.** One `Detector` trait, `detect()`
takes `SensorData::{Audio, Image, Pir}`, implementors match on the variant they care
about. Gives you a true `Vec<Box<dyn Detector>>` registry. Cost: runtime check for
what should be a compile-time guarantee (implementors must return `DetectError::
UnsupportedModality` or similar when handed the wrong variant); adding a modality
means editing the central enum (coupling).

**[DECIDED] Option B — generic supertrait + per-modality subtraits.** A common
`Detector` supertrait defines what is truly shared (identity, versioning, modality
reporting, health/readiness); `AudioDetector: Detector` and `VisionDetector: Detector`
add the strongly-typed modality-specific `detect()` method. BirdNET implements
`AudioDetector`, which transitively makes it a `Detector` — the conceptual unity is
expressed in the type system, not handwaved.

**On the runtime-vs-compile-time tradeoff.** The "cost" of Option C that I referred
to in iter. 5 isn't runtime performance — the enum match is negligible. The cost is
giving up compile-time type safety in exchange for a runtime capability
(`Vec<Box<dyn Detector>>`) that Phase 1 cannot use and does not need. When Phase 6
brings wasmtime plugins and we genuinely need runtime-polymorphic dispatch, we add a
thin `DynDetector` adapter at the plugin-loader boundary. Native Rust detectors
never pay that cost; the plugin world gets its flexibility behind an explicit,
well-scoped boundary. This is strictly better than paying the cost globally in
Phase 1 against a hypothetical future need.

**Fallback paths (re-evaluate after Phase 3).**

- If `AudioDetector` and `VisionDetector` end up sharing so much that duplication
  becomes the dominant pain point, **compact** — fold shared logic into `Detector`
  or collapse into a single trait with an associated input type.
- If they diverge further than expected (e.g., vision needs streaming-frame
  semantics that don't fit a single `detect()` call), **specialize further** — add
  additional methods or split into more specific subtraits.

Either evolution is backwards-compatible with the Phase 1 shape because all
consumers route through the supertrait or a specific subtrait by name, never by
unification assumptions.

**Next step.** Concrete `Detector` and `AudioDetector` signatures are co-designed
as a PR against the repo, not in this doc.

## 7. Repo structure (Phase 1 target)

Single crate, modules. Split into a workspace if and when friction appears.

```
muir/
├── Cargo.toml
├── LICENSE-APACHE
├── README.md
├── src/
│   ├── main.rs          # binary entrypoint
│   ├── lib.rs           # re-exports
│   ├── config.rs        # JSON loading via serde_json
│   ├── ingest/
│   │   ├── mod.rs
│   │   └── audio_file.rs
│   ├── inference/
│   │   ├── mod.rs       # defines `trait Detector`
│   │   └── birdnet.rs
│   ├── fusion/
│   │   └── mod.rs
│   └── sink/
│       ├── mod.rs
│       └── stdout.rs    # Phase 1: just print events
├── models/              # gitignored
├── fixtures/            # committed test audio (short, CC0/public-domain)
├── scripts/
│   └── fetch_birdnet.sh
├── benches/             # Criterion benchmarks (§4 principle 4)
└── docs/
    ├── design.md        # this file
    └── config-schema.json
```

## 8. Phase roadmap (local-first)

### Phase 1 — Local BirdNET (weeks 1–3)
Goal: `cargo run -- --config ./muir.json path/to/bird.wav` produces a structured list of
species detections on stdout.

- `cargo init`, Apache-2.0 license, `.gitignore`, initial commit
- Fetch BirdNET, confirm/execute ONNX conversion path, document in `scripts/fetch_birdnet.sh`
- Skeleton modules per repo structure above
- `config.rs` loads lat/lon/sensor-id from `muir.json`
- `ingest::audio_file` reads `.wav`, resamples to 32 kHz mono, yields non-overlapping 5 s windows (160,000 samples) for BirdNET v3.0
- `inference::Detector` trait (shape TBD per §6.11) + `inference::birdnet::BirdnetDetector` impl wrapping `birdnet_onnx::Classifier`
- `sink::stdout` prints detections as pretty JSON
- At least one integration test using a committed fixture in `fixtures/`
- CI: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, on push
- README with 5-line quickstart, model license note

### Phase 2 — Async + Oracle deploy + model footprint (weeks 4–5)
- Refactor file ingest to use `tokio::fs` and return a stream
- Bounded channels between stages (ingest → inference → fusion → sink)
- Cross-compile to aarch64 via `cross`
- Deploy to Oracle Always-Free ARM VM, run against the same fixture, verify identical
  output byte-for-byte
- Criterion benchmark harness for per-segment inference latency + memory on aarch64
- Benchmark v3.0 vs. v2.4 (and any INT8-quantized variant we can cheaply produce);
  pick the lighter/faster model per §6.2 and make it the default post-MVP

### Phase 3 — Camera + fusion (weeks 6–7)
### Phase 4 — Persistence + uplink (weeks 8–9)
### Phase 5 — LLM summarization (weeks 10–11)
### Phase 6 — Plugin architecture + dashboard (weeks 12–14)

## 9. Open questions (for next session)

Resolved in iter. 4 (retained here as a decision log):

- ~~Model version for MVP~~ — **v3.0** chosen; benchmark v2.4 later (§6.2).
- ~~`sensor_location` on Detection~~ — **kept as `Option<LatLon>`**; privacy handled by
  sink-layer redaction, not by omission (§6.6, §4 principle 8).
- ~~`.github/` discoverability scaffolding in Phase 1~~ — **deferred** as premature.
- ~~Detector trait sketch~~ — **Option B chosen** (generic supertrait + per-modality
  subtraits) with documented fallback paths. Concrete trait signatures get co-designed
  as the first PR against the repo, not in this doc.

Still open:

1. **Testing fixtures.** Where do we source a short, permissively licensed `.wav` with
   known bird calls for `fixtures/`? Xeno-canto has CC-licensed recordings; we need one
   with a clear license suitable for redistribution and a known species we can assert on
   in tests. Defer until we're writing the first integration test in Phase 1.

2. **Fusion timing windows (Phase 3, but worth noting).** When we add camera + fusion,
   what's the temporal window for correlating cross-modal detections? Too tight and we
   miss genuinely co-occurring events; too loose and we manufacture spurious events.
   Not a Phase 1 concern — noting it to revisit.

3. **Embeddings surface area (§6.2 v3.0 bonus).** Do we expose BirdNET v3.0's 1280-dim
   embeddings on `Detection`, or keep them internal to the detector for now? Exposing
   them unlocks similarity search later; storing them inflates the event log. Lean:
   keep internal for MVP; add as an opt-in field in Phase 3–4 if a real use case lands.

## 10. Glossary

- **BirdNET** — Cornell Lab of Ornithology's acoustic bird species classifier.
- **MegaDetector** — Microsoft AI for Earth's camera-trap animal/human/vehicle detector.
- **SMART** — Spatial Monitoring and Reporting Tool, NGO-operated ranger patrol software.
- **Wildlife Insights** — Google/Conservation International camera-trap data platform.
- **Arbimon** — Bioacoustic monitoring platform from Rainforest Connection.
- **Xeno-canto** — Community archive of CC-licensed bird recordings.
- **Gateway** — in Muir's context, the edge software that sits between sensors and
  upstream systems. Not to be confused with L4 networking gateway.
