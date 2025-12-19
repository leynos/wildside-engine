# Wildside recommendation engine roadmap

## Phase 1: Data foundation and core types

This phase focuses on establishing the data ingestion pipeline and defining the
core data structures of the engine.

- **Set up Project Structure**

  - [x] Create the repository root directory `wildside-engine` and initialize a
        virtual workspace:

    ```bash
    mkdir wildside-engine && cd wildside-engine
    git init
    cargo init --vcs git
    ```

  - [x] Replace the root `Cargo.toml` with a virtual workspace manifest (no
        `[package]`), defining members: `cargo new --lib wildside-core`,
        `cargo new --lib wildside-data`, and `cargo new --bin wildside-cli`.
  - [x] Configure the root `Cargo.toml` to define the workspace members
    (`wildside-core`, `wildside-data`, `wildside-cli`) and set `resolver = "2"`.

- **Define Core Domain Model**

  - [x] In `wildside-core`, define the public struct `PointOfInterest`
        with essential fields like `id`, `location: geo::Coord<f64>`, and
        `tags: HashMap<String, String>`.
  - [x] Define the `InterestProfile` struct to hold selected themes for visitors
        and their corresponding weights.
  - [x] Define the `Route` struct, containing an ordered `Vec<PointOfInterest>`
        and a `total_duration: std::time::Duration`.
  - [x] Define the `PoiStore` trait with methods like:
        <!-- markdownlint-disable-next-line MD013 -->
        `get_pois_in_bbox(&self, bbox: &geo::Rect<f64>) -> Box<dyn
        Iterator<Item = PointOfInterest> + Send + '_>`
  - [x] Define the `TravelTimeProvider` trait with a method
        <!-- markdownlint-disable-next-line MD013 -->
        `get_travel_time_matrix(&self, pois: &[PointOfInterest]) ->
        Result<TravelTimeMatrix, TravelTimeError>`
  - [x] Define the `Scorer` trait with a
        `score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32`
        method.
  - [x] Define the `Solver` trait with a
        `solve(&self, request: &SolveRequest) -> Result<SolveResponse, SolveError>`
        method.

- **Implement OSM PBF ingestion**

  - [x] In `wildside-data`, add `osmpbf` and `geo` as dependencies.
  - [x] Create a public function `ingest_osm_pbf(path: &Path)` that uses
        `osmpbf::par_map_reduce` to process a PBF file in parallel.
  - [x] Implement the logic to filter for relevant OSM elements (e.g., nodes and
        ways with specific tags like `historic`, `tourism`) and convert them
        into `PointOfInterest` instances.

- **Adopt GeoRust Primitives**

  - [x] Standardize on `geo::Coord` for all location data within the
        `PointOfInterest` struct.
  - [x] Create a function `build_spatial_index` that consumes an iterator of
        `PointOfInterest` values and returns a `SpatialIndex` backed by an
        R*-tree.
  - [x] Implement a `SqlitePoiStore` that loads a pre-built R*-tree from a file
        and uses it to implement the `get_pois_in_bbox` method efficiently.

- **Build Wikidata ETL Pipeline**

  - [x] In `wildside-data`, add `wikidata-rust`, `simd-json`, and
        `rusqlite` dependencies.
  - [x] Write a script that downloads the latest Wikidata JSON dump.
  - [x] Implement a parser that iterates through the dump, filters for entities
        linked from the ingested OSM data, and extracts relevant claims (e.g.,
        `P1435` for heritage status).
  - [x] Design and create a SQLite database schema (`pois.db`) to store these
        claims in an indexed and queryable format.
  - [ ] Emit progress and metrics during ETL runs (entities scanned, matched,
        inserted) and persist a summary log artefact.
  - [ ] Record dump provenance (download timestamp, checksum, source URL) in
        `pois.db` metadata tables and surface it via the CLI.
  - [ ] Add a Postgres load step that migrates schema changes and bulk-loads
        the enriched Wikidata attributes into `pois` (JSONB column or
        auxiliary tables) from `pois.db`.

- **Develop Initial CLI**

  - [x] In `wildside-cli`, use the `ortho-config` crate to define an `ingest`
        command with arguments for the OSM PBF and Wikidata dump file paths.
        (see `docs/ortho-config-users-guide.md`)
  - [x] Implement the command's handler to orchestrate the full pipeline: call
        `ingest_osm_pbf`, then the Wikidata ETL process, and finally
        `build_spatial_index`, saving the resulting `pois.db` and `pois.rstar`
        files.

## Phase 2: Scoring and personalization

This phase implements the core logic that gives the engine its intelligence.

- **Implement Global Popularity Scorer**

  - [x] Create the `wildside-scorer` crate.
  - [x] Implement an offline process that iterates through `pois.db`, calculates
        a popularity score for each POI based on its sitelink count and
        heritage status, and normalizes the scores.
  - [x] Serialize the resulting `HashMap<PoiId, f32>` of scores to a compact
        binary file (`popularity.bin`) using a library like `bincode`.

- **Implement User Relevance Scorer**

  - [x] Implement the `score` method of the `Scorer` trait.
  - [x] The method will receive a `PointOfInterest` and an `InterestProfile`.
  - [x] It will perform fast, indexed lookups against `pois.db` to check for
        Wikidata properties matching selected interests.
  - [x] It will combine these matches with the pre-calculated global
        popularity score loaded from `popularity.bin`.

- **Define Stable API**

- [x] In `wildside-core`, define the `SolveRequest` struct with public
        fields for `start: geo::Coord`, `duration_minutes: u16`,
        `interests: InterestProfile`, and a `seed: u64` for reproducible
        results. Status: includes an optional `max_nodes` pruning hint to cap
        candidate selection when callers supply it.
  - [x] Define the `SolveResponse` struct to include the final `Route`, the
    total `score`, and a `Diagnostics` struct for metrics like solve time and
    number of candidates.

## Phase 3: The orienteering problem solver

This phase tackles the complex route-finding algorithm.

- **Implement Native VRP Solver**

  - [x] Create the `wildside-solver-vrp` crate with a dependency on
        `vrp-core`.
  - [x] Create a `VrpSolver` struct that implements the `Solver` trait from the
        core crate.
  - [x] The `solve` method will first select candidate POIs from the `PoiStore`.
  - [x] It will then fetch the travel time matrix for these candidates from the
        `TravelTimeProvider`.
  - [x] It will configure the `vrp-core` problem and objective function to
        maximize the total collected score within the given time budget.
  - [x] Finally, it will run the `vrp-core` solver and transform the result into
        a `SolveResponse`.

- **Implement Travel Time Provider**

  - [x] Create a `HttpTravelTimeProvider` struct that implements the
        `TravelTimeProvider` trait.
  - [x] Using `tokio` and `reqwest`, implement the `get_travel_time_matrix`
        method to make concurrent requests to an external OSRM API's `table`
        service.

- **Integrate Solver into CLI**

  - [ ] Add a `solve` command to `wildside-cli` that accepts a path to a
        JSON file.
  - [ ] The command will deserialize the JSON into a `SolveRequest`, instantiate
        the necessary components (store, scorer, solver), call the solver, and
        print the resulting `SolveResponse` as formatted JSON.

## Phase 4: Testing, deployment, and polish

This phase ensures the engine is robust, reliable, and ready for integration.

- **Establish Testing Discipline**

  - [ ] Create a `tests/golden_routes` directory with small, well-defined
    problem instances and their known optimal solutions in JSON format to act
    as regression tests.
  - [ ] Use `proptest` to write property-based tests for the solver, asserting
        invariants like "total route duration must not exceed Tmax" and "route
        must start and end at the same point".
  - [ ] Use `criterion` to create a benchmark suite that measures the P95 and
    P99 solve times for various problem sizes (e.g., 50, 100, 200 candidate
    POIs).

- **Implement Feature Flags**

  - [ ] In the root `Cargo.toml`, define features like `solver-vrp`,
        `solver-ortools`, and `store-sqlite`.
  - [ ] Forward feature flags from member crates using `[features]` and
        `dep:`-scoped entries to ensure a single source of truth.

    ```toml
    # In the root Cargo.toml
    [dependencies]
    wildside-solver-vrp = { version = "0.1", optional = true, default-features = false }
    wildside-solver-ortools = { version = "0.1", optional = true, default-features = false }
    wildside-data = { version = "0.1", optional = true, default-features = false }

    [features]
    solver-vrp = ["dep:wildside-solver-vrp"]
    solver-ortools = ["dep:wildside-solver-ortools"]
    # Enable the optional dependency and forward its `sqlite` feature
    store-sqlite = ["dep:wildside-data", "wildside-data/sqlite"]
    ```

  - [ ] Use `#[cfg(feature = "...")]` attributes to conditionally compile the
        different solver and store implementations.

- **Finalize Licensing and Versioning**

  - [ ] Add the ISC `LICENSE` file to the root of the workspace and to each
        crate's `Cargo.toml`.
  - [ ] Initialize a `CHANGELOG.md` file at the root, documenting the initial
        `0.1.0` feature set.

- **(Optional) Implement OR-Tools Solver**

  - [ ] Create a `wildside-solver-ortools` crate, conditionally compiled
        via the `ortools` feature flag.
  - [ ] Add a dependency on a suitable OR-Tools wrapper crate.
  - [ ] Implement the `Solver` trait using the CP-SAT solver, mapping the
        Orienteering Problem to its constraint model.
