# Section 5: Implementation, Testing, and Deployment Strategy

This final section consolidates the architectural decisions into an actionable
plan covering packaging, testing, and versioning.

## 5.1. Packaging, Versioning, and Features

The engine will be structured for robust dependency management and deployment.

- **Licensing:** All engine crates (`wildside-*`) will be licensed under
  the permissive **ISC license**, satisfying the project's legal requirements
  while being clear and concise.

- **Versioning:** Each crate within the workspace will be independently
  versioned using Semantic Versioning. This allows for stable, predictable
  updates for consumers of the library. A `CHANGELOG.md` file will be
  maintained from the start.

- **Feature Flags:** The engine uses feature flags so consumers can select the
  solver and store implementations they need, while keeping the default build
  conservative and easy to ship.

  - `default-features = ["solver-vrp", "store-sqlite", "serde"]`: The default
    build includes the native VRP solver, the SQLite POI store, and serde
    support for request/response types.

  - `solver-vrp`: Enables the native Rust solver backed by `vrp-core` and is
    preferred when multiple solver features are enabled.

  - `solver-ortools`: Enables the optional OR-Tools solver backend. The current
    implementation is a placeholder until CP-SAT integration lands.

  - `store-sqlite`: Enables the SQLite-backed POI store and the spatial index
    format used to load persisted artefacts.

  - Future optional features may include `rocksdb`, `serde-bincode`, and `wasm`
    once alternative stores or targets are implemented.

## 5.2. A Non-Negotiable Testing and Benchmarking Discipline

For a library intended to be a reliable core component, a rigorous testing
discipline is non-negotiable.

- **Golden Tests:** Small graph instances (5–20 POIs) with hand-verified or
  exhaustively calculated optimal solutions will be used to validate the
  correctness of the solver's output. See §5.2.1 for implementation details.

- **Property Tests:** The `proptest` crate will be used to assert invariants
  that must always hold true (e.g., a route must always start and end at the
  depot, the total time must never exceed the budget, no duplicate POIs are
  visited).

- **Performance Benchmarks:** The `criterion` crate will be used to create a
  performance suite that runs on CI. This will track the p95 wall-time for
  solving problems of various sizes (e.g., N=50, 100, 200 candidates) to
  prevent performance regressions.

- **Fuzz Testing:** The scorer's theme mapping and the data ingestion logic's
  handling of Wikidata claims will be fuzzed to ensure resilience against
  unexpected or malformed data.

- **API Contract Tests:** The serialization format of `SolveRequest` and
  `SolveResponse` will be tested to ensure the "wire schema" remains stable
  across versions, preventing breaking changes for consumers.

### 5.2.1. Golden routes implementation

Golden route tests live in `wildside-solver-vrp/tests/golden_routes/`. Each
test case is stored as a JSON file containing a complete problem instance and
its expected solution. The schema captures:

- `pois`: Array of POIs with id, coordinates, and tags.
- `travel_time_matrix_seconds`: Integer matrix of travel times (avoids float
  precision issues).
- `request`: A complete `SolveRequest` specification including start/end
  coordinates, duration, interest profile, seed, and optional max_nodes.
- `expected`: The expected route POI IDs in order, score range (min/max to
  accommodate metaheuristic variance), and invariants such as budget compliance.

The test infrastructure includes:

- `FixedMatrixTravelTimeProvider`: A `TravelTimeProvider` implementation that
  returns a caller-supplied matrix verbatim. This enables fully deterministic
  tests without external routing dependencies.

- rstest parameterised unit tests (`golden_routes.rs`): A single test function
  iterates over all JSON fixtures, loads each problem instance, constructs the
  solver with the fixed matrix, and asserts that the solution matches
  expectations.

- rstest-bdd behavioural tests (`golden_routes_behaviour.rs`): Gherkin scenarios
  in `features/golden_routes.feature` document solver behaviour at a higher
  abstraction level, covering happy paths and edge cases.

Design decisions:

| Decision                             | Rationale                                    |
| ------------------------------------ | -------------------------------------------- |
| Travel times as integer seconds      | Avoids floating-point precision issues       |
| Score ranges instead of exact values | Accommodates metaheuristic variance          |
| `FixedMatrixTravelTimeProvider`      | Enables fully deterministic tests            |
| rstest `#[case]` parameterization    | Single test function scales to many fixtures |
| Separate BDD layer                   | Documents behaviour at higher abstraction    |
| JSON for test data                   | Human-readable, easy to maintain             |

### 5.2.2. Property-based testing implementation

Property-based tests live in `wildside-solver-vrp/tests/property_tests.rs` and
use the `proptest` crate to assert invariants that must hold for all valid
solver inputs. The test suite generates random but valid `SolveRequest`
instances and verifies:

1. **Budget compliance:** Route duration never exceeds the time budget (Tmax).
2. **No duplicates:** Each POI appears at most once in the route.
3. **Score validity:** Scores are non-negative and finite.
4. **Constraint adherence:** `max_nodes` limits are respected.
5. **POI validity:** All route POIs exist in the candidate set.
6. **Point-to-point validity:** Routes with distinct end locations maintain
   all core invariants.
7. **Empty candidates:** When no candidates match, an empty route with zero
   score is returned.

The tests use `UnitTravelTimeProvider` from `wildside-core::test_support`,
which generates correctly sized travel time matrices dynamically based on the
actual number of candidates after filtering. This avoids the complexity of
pre-computing matrices for variable-sized candidate sets.

Design trade-offs:

| Decision                          | Rationale                                           |
| --------------------------------- | --------------------------------------------------- |
| Small POI sets (3-10 nodes)       | Fast execution while exercising core logic          |
| `UnitTravelTimeProvider`          | Dynamic matrices adapt to filtered candidate counts |
| `ProptestConfig::with_cases(100)` | Balances coverage against CI execution time         |
| Seed variation only               | Fixed POI geometry, random seeds exercise heuristic |
| Support module separation         | `proptest_support.rs` keeps strategies reusable     |

## 5.3. Repository and Migration Strategy

The project will begin in a single Git repository configured as a Cargo
workspace. This simplifies initial development while still enforcing the clean
separation between crates. If the engine later needs to be consumed by external
partners, support non-Rust bindings (e.g., Python via PyO3), or adopt a
different release cadence from the main application, it can be promoted to its
own repository with no code churn, as the boundaries are already established.

The migration from an initial "engine-in-app" prototype to the final library
structure follows a clear path:

1. Extract all domain types into `wildside-core` and update the
   application to import from the new crate.

2. Move scoring and solver logic into the `wildside-scorer` and
   `wildside-solver-vrp` crates, implementing the traits from `core`. The
   application code becomes a thin adapter.

3. Introduce the `wildside-cli` and integrate it into the CI pipeline
   for repeatable data ingestion and performance snapshots.

4. Change the application's dependency on the engine crates from a local
   path-based dependency to a versioned one, potentially using a private crate
   registry or Git tags.

This structured approach provides immense benefits in cohesion, replaceability
of components (like the solver), reusability for other applications or
bindings, and overall reliability.
