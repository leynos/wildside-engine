# Section 3: A Library-First Architecture for the Wildside Engine

The previous analysis focused on the "what" of the data pipeline; this section
defines the "how" of the application structure. To ensure maintainability,
testability, and reusability, the engine will not be a monolithic service.
Instead, it will be engineered as a **versioned Rust library composed of
several distinct crates**, with a minimal command-line interface (CLI) for
offline tasks.

## 3.1. The Core Principle: Engine as a Library

Instead of embedding the core logic directly within a web application, the
engine will be developed as a standalone set of crates with a stable, semantic
API. The web application (e.g., an Actix or Axum server) becomes a thin client
that depends on the engine library. This approach avoids a tangle of internal
modules and promotes a clean separation of concerns. It also defers the
complexities of a network service architecture (latency, auth, retries, new
failure modes) until they are truly needed, allowing the initial focus to be on
the core logic.

## 3.2. Proposed Crate Layout and Separation of Concerns

A Rust workspace will be used to manage the engine's components, enforcing
clear boundaries while allowing for atomic changes across crates when necessary.

- `wildside-core`: This crate is the heart of the engine. It contains
  the pure domain model and traits, with no I/O or specific framework
  dependencies (i.e., it is `#![no_std]` compatible where possible, without
  `tokio` or `actix`).

  - **Types:** `PointOfInterest`, `InterestProfile`, `Score`, `Route`,
    `Instruction`.

  - **Traits:** Defines the core abstractions: `PoiStore` (for read-only POI
    data access), `TravelTimeProvider` (for distance calculations), `Scorer`,
    and `Solver`.

  - This crate is deterministic and side-effect free, making it easy to test
    rigorously with property-based testing and fuzzing.

- `wildside-data`: Contains the ETL logic and data adapters.

  - Implements the `PoiStore` trait from the core crate.

  - Handles OSM PBF ingestion (using `osmpbf`), Wikidata dump parsing, and
    building the data artefacts (e.g., SQLite/RocksDB stores and the `rstar`
    index).

- (Planned) `wildside-scorer`: Implements the `Scorer` trait.

  - Contains the logic for both the offline pre-computation of global
    popularity scores and the per-request calculation of user relevance.

- (Planned) `wildside-solver-vrp`: The default, native Rust implementation of
  the `Solver` trait, using the `vrp-core` library.

- (Planned) `wildside-solver-ortools`: An optional implementation of the
  `Solver` trait, using bindings to Google's CP-SAT solver. This would be
  enabled via a feature flag for users who require its specific performance
  characteristics and are willing to manage the C++ dependency.

- `wildside-cli`: A small command-line application for operational tasks.

  - `ingest`: Runs the full ETL pipeline from `wildside-data` to build
    the necessary data artefacts.

    The command now uses `ortho-config` to fan in configuration sources. The
    `--osm-pbf` and `--wikidata-dump` flags map to
    `WILDSIDE_CMDS_INGEST_OSM_PBF` and `WILDSIDE_CMDS_INGEST_WIKIDATA_DUMP`
    environment variables, so CI pipelines and operators can set defaults once
    rather than repeating long paths. Until the downstream orchestration hooks
    are implemented, the CLI validates that both files exist and surfaces clear
    `MissingArgument` or `MissingSourceFile` errors. This keeps the UX stable
    while further pipeline work completes.

  - (Planned) `score`: Triggers the batch computation of global popularity
    scores.

  - `solve`: Runs the route solver from the command line by loading a
    JSON-encoded `SolveRequest` and printing a formatted JSON `SolveResponse`.

    The command loads pre-built artefacts (`pois.db`, `pois.rstar`,
    `popularity.bin`) from the current directory by default, or from an
    explicit `--artefacts-dir`. Each artefact path can be overridden via CLI
    flags/config layers, and the OSRM base URL can be customized via
    `--osrm-base-url`.

    Design decision: the request JSON contains only the domain `SolveRequest`;
    data and infrastructure paths are resolved via CLI/config to avoid
    embedding environment-specific absolute paths inside otherwise portable
    golden request files.

## 3.3. A Stable, Performant, and Boring API Surface

The public API of the engine should be simple, stable, and predictable. The
primary interaction will be through a request/response model.

**Input Structure:**

```rust
pub struct SolveRequest {
    pub start: geo::Coord,      // f64 lat/lon
    pub end: Option<geo::Coord>, // Optional end lat/lon for point-to-point routing
    pub duration_minutes: u16,  // Tmax
    pub interests: InterestProfile,
    pub seed: u64,              // For deterministic, reproducible heuristic runs
    pub max_nodes: Option<u16>, // Optional pruning hint for candidate search
}
```

**Output Structure:**

```rust
pub struct Diagnostics {
    pub solve_time: Duration,       // Time taken to produce the solution
    pub candidates_evaluated: u64,  // Number of candidate POIs evaluated
}

pub struct SolveResponse {
    pub route: Route,           // Ordered list of coords + POI IDs
    pub score: f32,             // Total collected score for the route
    pub diagnostics: Diagnostics, // Telemetry from the solve operation
}
```

The inclusion of a `seed` in the request is critical for reliability. It makes
the heuristic solver's output reproducible, which is invaluable for testing,
benchmarking, and diagnosing production incidents. A lightweight
`SolveRequest::validate` helper enforces the core invariant: a duration of zero
minutes is rejected with `SolveError::InvalidRequest`. The optional `max_nodes`
pruning hint must be greater than zero when supplied; `None` leaves solver
implementations free to choose candidate limits. When provided, the optional
`end` location enables point-to-point routing: solvers should model tours as
starting at `start` and finishing at `end` rather than returning to the start
location. The `Diagnostics` struct captures solver telemetry, including elapsed
time and the number of candidates evaluated, enabling performance monitoring
and debugging.

## 3.4. Data and Computation Boundaries: Offline vs. Online

A strict separation between offline preparation and online serving is essential
for performance and scalability.

- **Offline Path:** The `wildside-cli` is used to execute the idempotent
  ETL process. This process takes raw OSM and Wikidata data and produces a set
  of optimized, read-only artefacts:

  - `pois.db`: An SQLite (or RocksDB) file containing the enriched POI data,
    indexed for fast lookups. Schema changes are backward compatible within a
    major release.

  - `pois.rstar`: A serialized R\*-tree file for fast spatial queries, which
    can be loaded into memory using memory-mapping (e.g., with `memmap2`) for
    near-instant startup. The layout remains stable across 0.x releases.

  - `popularity.bin`: A compact binary file of pre-calculated global
    popularity scores. The structure remains stable across 0.x releases; bump
    the artefact header version per §3.4.1 when making breaking changes.

The `wildside` CLI now wires these stages together: the `ingest` command
validates input paths, streams the PBF to derive POIs, writes `pois.db`
(creating parent directories when required), extracts linked claims from plain
JSON or `.bz2` Wikidata dumps, and serializes the R\*-tree to `pois.rstar`.
When no POIs carry a `wikidata` tag, the ETL is skipped but the claims schema
is still initialized to keep artefact shapes stable. Output paths default to
the current working directory and can be overridden via `--output-dir`.
Filesystem access during these steps relies on `cap-std`'s `fs_utf8` module and
`camino` paths to ensure UTF-8-safe handling and to keep the pipeline ready for
capability-based sandboxing.

### 3.4.1. Artefact versioning and migration

Embed a fixed header: 4-byte ASCII magic "WSPI", u16 major, u16 minor, u8
flags, all little-endian. Bump MAJOR for incompatible changes; bump MINOR for
backward-compatible additions. Readers MUST refuse unknown MAJOR versions and
MAY accept newer MINOR versions. Provide a `wildside-cli migrate` subcommand
that detects legacy headers, runs the appropriate migrator, and emits a clear
error with expected vs found MAJOR.MINOR on mismatch.

- **Online Path:** The core engine library, when used by the web app, interacts
  *only* with these read-only artefacts. This design choice means the engine
  itself is side-effect free during a request. It allows application instances
  to be scaled horizontally without needing complex shared state or writeable
  database connections, dramatically simplifying deployment and improving
  robustness.
