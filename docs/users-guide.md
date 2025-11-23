# Wildside engine user guide

The Wildside engine is delivered as a collection of Rust crates focused on
serving recommendation and routing scenarios for walking tours. This guide
summarises the functionality that is currently implemented and explains how to
consume the public APIs exposed by the workspace.

## Crate layout

- `wildside-core`: library crate exposing the core domain model, storage
  abstractions, scoring hooks, solver contract, and travel-time interfaces.
- `wildside-engine`: convenience facade that currently exposes a `greet`
  helper. Downstream applications should depend on the specific feature crates
  described in this guide until the facade is expanded.
- `wildside-cli`: offline tooling that runs the `ingest` pipeline to build
  `pois.db` and `pois.rstar` from an OSM PBF plus a Wikidata dump.

## Core data model

The `wildside-core` crate defines the types that form the API surface for
consumers.

### Points of interest

`PointOfInterest` represents an attraction worth visiting and carries a unique
identifier, WGS84 coordinate, and a free-form map of tags. Helper constructors
`new` and `with_empty_tags` simplify creation, and the type implements
`RTreeObject` so it can be indexed directly. Spatial lookup is exposed through
`SpatialIndex`, which supports iteration, bounding-box queries, and index
construction via the `build_spatial_index` helper.[^1]

### Themes and interest profiles

`Theme` enumerates supported interest categories and provides string
conversions for serialisation and parsing. `InterestProfile` stores per-theme
weights in the `0.0..=1.0` range and offers validated setters (`set_weight`,
`try_set_weight`) and chaining via `with_weight`. Invalid weights raise
`WeightError` (`OutOfRange` or `NonFinite`).[^2][^3]

### Routes

`Route` captures an ordered list of points of interest plus a caller-supplied
`Duration`. Use `Route::new` to build routes returned from solvers and
`Route::empty` for initialisation. The route does not infer travel time;
callers must provide the aggregate duration explicitly.[^4]

## Scoring contract

The `Scorer` trait maps a `PointOfInterest` and `InterestProfile` to a `f32`
score. Implementations must be `Send + Sync`, return deterministic,
non-negative, finite values, and should normalise scores to `0.0..=1.0`.
`Scorer::sanitise` is provided to clamp or reset invalid values.[^5]

## Solver contract

Tour construction is delegated to the `Solver` trait. Consumers build a
`SolveRequest` that includes a start coordinate, visit duration (minutes), an
interest profile, and a random seed for deterministic behaviour. Call
`SolveRequest::validate` to enforce the implemented invariants: non-zero
duration and finite coordinates. Successful solvers return `SolveResponse`,
which packages the chosen `Route` and its aggregate score. Invalid inputs must
produce `SolveError::InvalidRequest` instead of panicking.[^6]

## Point-of-interest storage

The `PoiStore` trait abstracts read-only access to points of interest via
bounding-box queries.[^7] Implementations must accept rectangles in longitude,
latitude order (WGS84) and treat boundary points as contained. The default
store is `SqlitePoiStore`, which opens two artefacts: a read-only SQLite
database and a serialised R\*-tree. The loader verifies both files by reading a
`WSPI` magic header, checking the format version (`2`), and ensuring that every
indexed point exists in the database. Failing checks raise
`SqlitePoiStoreError`, covering problems such as missing records, malformed
JSON tag payloads, and I/O or SQLite errors.[^8]

## Travel-time providers

Travel-time lookups are pluggable via the `TravelTimeProvider` trait, which
returns an `n×n` adjacency matrix (`TravelTimeMatrix`). Implementations must
report `TravelTimeError::EmptyInput` when called with an empty slice.[^9] The
library ships with `UnitTravelTimeProvider` behind the `test-support` feature
to simplify integration testing.[^10]

## Test support utilities

Enabling the `test-support` feature unlocks helpers intended for integration
and unit tests:

- `MemoryStore`: in-memory `PoiStore` performing linear scans for small
  datasets.[^11]
- `UnitTravelTimeProvider`: deterministic provider returning one-second edges,
  useful for reproducible solver fixtures.[^12]
- `TagScorer`: reference `Scorer` that sums theme weights based on tag keys,
  demonstrating how to convert tags into scores.[^13]

## Error handling summary

API consumers should handle the following error types surfaced by the library:

- `WeightError`: returned by `InterestProfile::try_set_weight` when weights are
  out of range or non-finite.[^14]
- `SolveError`: produced by solvers when requests violate
  invariants.[^15]
- `TravelTimeError`: emitted by travel-time providers for invalid input such as
  empty POI slices.[^16]
- `SqlitePoiStoreError`: covers storage and validation failures encountered when
  opening SQLite-backed stores.[^17]

## Typical integration flow

The snippet below sketches how an API consumer might compose the building
blocks. It assumes the presence of application-specific scorer and solver
implementations.

```rust
use geo::{Coord, Rect};
use wildside_core::{
    InterestProfile, PointOfInterest, PoiStore, Scorer, SolveRequest, Solver,
    SqlitePoiStore, Theme, TravelTimeProvider,
};

fn plan_visit(
    store: &SqlitePoiStore,
    scorer: &(impl Scorer + ?Sized),
    solver: &(impl Solver + ?Sized),
    travel_times: &(impl TravelTimeProvider + ?Sized),
) -> Result<(), Box<dyn std::error::Error>> {
    let bbox = Rect::new(
        Coord { x: -0.2, y: 51.45 },
        Coord { x: -0.1, y: 51.55 },
    );
    let pois: Vec<PointOfInterest> = store.get_pois_in_bbox(&bbox).collect();
    if pois.is_empty() {
        println!("No points of interest found inside the bounding box");
        return Ok(());
    }
    let travel_matrix = travel_times.get_travel_time_matrix(&pois)?;

    let mut profile = InterestProfile::new();
    profile.set_weight(Theme::History, 0.8);
    profile.set_weight(Theme::Art, 0.6);

    let request = SolveRequest {
        start: Coord { x: -0.15, y: 51.5 },
        duration_minutes: 180,
        interests: profile.clone(),
        seed: 42,
    };
    request.validate()?;

    let response = solver.solve(&request)?;
    let selection_scores: Vec<f32> = response
        .route
        .pois()
        .iter()
        .map(|poi| scorer.score(poi, &profile))
        .collect();
    let total_score: f32 = selection_scores.iter().sum();

    println!("Route duration: {:?}", response.route.total_duration());
    println!("Solver-reported score: {}", response.score);
    println!("Recomputed score: {total_score}");
    println!("Matrix size: {}×{}", travel_matrix.len(), travel_matrix[0].len());
    Ok(())
}
```

This workflow highlights the responsibilities enforced by the implemented API:
load POIs through a store, compute travel times, configure user interests,
validate solver input, and rely on deterministic scoring and solving contracts
for repeatable results.

[^1]: <../wildside-core/src/poi.rs#L13-L113>
[^2]: <../wildside-core/src/theme.rs#L1-L83>
[^3]: <../wildside-core/src/profile.rs#L1-L98>
[^4]: <../wildside-core/src/route.rs#L1-L75>
[^5]: <../wildside-core/src/scorer.rs#L1-L55>
[^6]: <../wildside-core/src/solver.rs#L1-L69>
[^7]: <../wildside-core/src/store.rs#L1-L220>
[^8]: <../wildside-core/src/store.rs#L361-L404>
[^9]: <../wildside-core/src/travel_time/provider.rs#L1-L59>
[^10]: <../wildside-core/src/test_support.rs#L1-L99>
[^11]: <../wildside-core/src/test_support.rs#L7-L56>
[^12]: <../wildside-core/src/test_support.rs#L100-L134>
[^13]: <../wildside-core/src/test_support.rs#L135-L151>
[^14]: <../wildside-core/src/profile.rs#L21-L98>
[^15]: <../wildside-core/src/solver.rs#L33-L69>
[^16]: <../wildside-core/src/travel_time/error.rs#L1-L14>
[^17]: <../wildside-core/src/store.rs#L28-L164>
