# Section 4: The Solver Implementation — Abstracted and Replaceable

This section addresses the most computationally intensive component of the
Wildside engine: solving the Orienteering Problem (OP). The library-first
architecture allows us to abstract the solver behind a trait, making the
specific implementation a configurable choice.

## 4.1. The `Solver` Trait: A Common Interface

The `wildside-core` crate will define a `Solver` trait. This trait will have a
single primary method,
`solve(request: &SolveRequest) -> Result<SolveResponse, SolveError>`, which
encapsulates the entire process of finding an optimal route. The trait is
object-safe and keeps the solver synchronous for embeddability. Implementations
must be `Send + Sync` so solvers can run on threaded callers. They should call
`request.validate()` (or enforce the same invariants) so that zero-duration
requests and non-finite start coordinates yield `SolveError::InvalidRequest`.
Implementations must avoid shared mutable state or use proper synchronization
to maintain thread safety. This abstraction is the key to making the engine
flexible and future-proof.

## 4.2. Recommended Native Rust Solution with `vrp-core`

For the initial implementation, a native Rust solution is strongly recommended.
The `reinterpretcat/vrp` project, specifically the `vrp-core` crate, is a
mature, well-documented, and permissively licensed (Apache-2.0) library for
solving rich Vehicle Routing Problems.13 The primary benefits of this approach
are a seamless development experience, guaranteed memory safety for the core
logic, and a simplified build process managed entirely by Cargo.

The Orienteering Problem is a known "routing problem with profits," which can
be modeled directly using the `vrp-core` API. The solver's objective function
will be configured to maximize the total collected `Score(POI)` from visited
"jobs" (the POIs), subject to the user's time budget (Tmax​). The library's
powerful built-in metaheuristics will efficiently find a high-quality route
within the required few-second timeframe.15 This implementation will live in
the `wildside-solver-vrp` crate.

### 4.2.1. Implementation notes

The first-cut `wildside-solver-vrp` implementation makes the following
pragmatic choices:

- Candidate search uses a rectangular bounding box derived from
  `duration_minutes` and an assumed average walking speed of 5 km/h, using a
  coarse conversion of 111 km per latitude degree. The box is anchored on
  `SolveRequest::start`, and if `SolveRequest::end` is provided, it expands to
  cover both the start and end points. This keeps selection synchronous and
  deterministic; callers can override the speed via `VrpSolverConfig`.

- Candidates are scored using the injected `Scorer` and sorted by score
  (descending, POI id tie-break). The optional `max_nodes` hint truncates this
  list before routing.

- The VRP model uses a single vehicle starting at the depot with an end time
  equal to the request budget in seconds. By default, the vehicle returns to
  the depot, but when `SolveRequest::end` is set, the vehicle ends at that
  distinct location (point-to-point routing). Service times at POIs are assumed
  to be zero for now.

- A custom `vrp-core` objective minimizes the negative sum of per-job scores.
  This is equivalent to maximizing total collected score, with travel time
  minimization applied as a secondary objective. Unassigned jobs carry no
  explicit penalty beyond these objectives.

- Until `SolveError` gains richer variants for routing failures, any failure in
  candidate routing, matrix acquisition, or `vrp-core` modelling is surfaced as
  `SolveError::InvalidRequest`.

- The request seed is not yet threaded into `vrp-core`'s random environment.
  Deterministic seeding will be added once the upstream API exposes a stable
  hook.

## 4.3. Optional High-Performance Backend: `wildside-solver-ortools`

To allow for future performance comparisons or to meet extreme optimization
requirements, a second implementation of the `Solver` trait can be provided in
the `wildside-solver-ortools` crate. This would use bindings to Google's highly
optimized CP-SAT solver, such as the `cp_sat` crate.

This approach offers potentially world-class performance but comes at the cost
of significant build and deployment complexity, requiring a C++ compiler and a
system-level installation of the OR-Tools library.16 By placing this
implementation behind a feature flag, consumers of the Wildside engine can
choose to opt into this complexity only if they absolutely need it, without
burdening the default setup.

## 4.4. The `TravelTimeProvider` boundary

A critical prerequisite for any VRP solver is the travel time matrix. The
solver itself is an abstract mathematical engine; it requires an external
component to provide the walking time between every pair of candidate POIs.

This is handled by the synchronous `TravelTimeProvider` trait defined in
`wildside-core`. The solver supplies a list of POIs including the start,
candidate POIs, and (when present) the distinct end location so that the matrix
covers all required legs. The trait has the signature:
<!-- markdownlint-disable-next-line MD013 -->
`fn get_travel_time_matrix(&self, pois: &[PointOfInterest]) -> Result<TravelTimeMatrix, TravelTimeError>`.
 Keeping the solver synchronous preserves object safety and makes the core
embeddable.

The recommended implementation will be an adapter that makes API calls to an
external, open-source routing engine like OSRM or Valhalla, running as a
separate microservice. This adapter will use `tokio` and an HTTP client like
`reqwest` to perform these network calls. This design keeps the core library
free of a specific async runtime, making it more broadly embeddable, while
still allowing it to communicate with the necessary external services.

### 4.4.1. HttpTravelTimeProvider implementation

The `HttpTravelTimeProvider` struct in `wildside-data::routing` implements the
`TravelTimeProvider` trait using the OSRM Table API. Key design decisions:

- **Synchronous trait, async internals:** The trait method is synchronous, but
  HTTP calls are inherently async. The implementation owns a current-thread
  Tokio runtime that is reused across calls, avoiding the overhead of creating
  a new runtime per request. When called from within an existing multithreaded
  Tokio runtime (detected via `Handle::try_current()` and `RuntimeFlavor`), it
  uses that runtime's handle with `block_in_place` to avoid nested runtime
  panics. When called from within a `current_thread` runtime, it falls back to
  using its own internal runtime to avoid the panic `block_in_place` would
  cause; however, this may lead to deadlocks if the caller's runtime is driving
  IO or timers that the request depends on.

- **OSRM Table API:** The provider calls `GET /table/v1/walking/{coordinates}`
  where coordinates are semicolon-separated `lon,lat` pairs. The response
  contains a `durations` array with travel times in seconds.

- **Unreachable pairs:** OSRM returns `null` for coordinate pairs where no
  route exists. These are mapped to `Duration::MAX` to indicate unreachable
  routes, allowing the solver to handle them appropriately.

- **Error handling:** The `TravelTimeError` enum includes variants for HTTP
  errors (`HttpError`), network failures (`NetworkError`), timeouts
  (`Timeout`), parse errors (`ParseError`), and service-level errors
  (`ServiceError`). All variants are marked `#[non_exhaustive]` for future
  expansion.

- **Configuration:** `HttpTravelTimeProviderConfig` supports customizing the
  base URL, request timeout, and user agent string via a builder pattern.

- **Fallible construction:** The `new()` and `with_config()` constructors return
  `Result<Self, ProviderBuildError>` to propagate HTTP client or Tokio runtime
  build failures instead of panicking.

- **Testing:** A `StubTravelTimeProvider` in `routing::test_support` allows
  unit and behavioural tests to verify provider consumers without requiring a
  running OSRM service. BDD scenarios cover happy paths and error conditions.
