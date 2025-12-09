# Wildside Recommendation Engine

*Walking tours, powered by open data.*

The Wildside engine generates personalized walking tours by combining
[OpenStreetMap](https://www.openstreetmap.org/) geospatial data with
[Wikidata](https://www.wikidata.org/) semantic information. Give it a starting
point, a time budget, and your interests—it'll find the most rewarding route
for you.

At its heart, the engine solves the
[Orienteering Problem](https://en.wikipedia.org/wiki/Orienteering_problem):
selecting which points of interest to visit (and in what order) to maximize
your enjoyment within the time you have. Think of it as a very enthusiastic
local guide who's read every Wikipedia article and knows exactly what you'd
love to see.

## Status

We're actively building this! The data foundation and scoring layers are in
place; the route optimization solver is next on the list. See
[docs/roadmap.md](docs/roadmap.md) for the full picture.

## Crate overview

The engine is organized as a Cargo workspace with focused, single-purpose
crates:

| Crate             | What it does                                                   |
| ----------------- | -------------------------------------------------------------- |
| `wildside-core`   | Domain model, traits, and abstractions—the shared vocabulary   |
| `wildside-data`   | ETL pipeline for ingesting OSM and Wikidata dumps              |
| `wildside-scorer` | Popularity and user-relevance scoring                          |
| `wildside-cli`    | Command-line tool for running the ingestion pipeline           |
| `wildside-fs`     | Filesystem abstraction layer                                   |

## Quick start

You'll need the Rust toolchain (see `rust-toolchain.toml` for the exact
version).

```sh
# Build the workspace
make build

# Run the test suite
make test

# Check formatting and lints
make check-fmt && make lint
```

To ingest data and build the POI database:

```sh
cargo run -p wildside-cli -- ingest \
  --osm-pbf path/to/region.osm.pbf \
  --wikidata-dump path/to/wikidata-dump.json.bz2 \
  --output-dir ./data
```

This produces `pois.db` (SQLite database), `pois.rstar` (spatial index), and
`popularity.bin` (precomputed scores)—the artefacts consumed at runtime.

## Documentation

For API details, usage patterns, and integration guidance, see the
[User Guide](docs/users-guide.md). The
[Design Document](docs/wildside-engine-design.md) covers architectural
decisions and implementation rationale.

## Licence

[ISC](LICENSE)
