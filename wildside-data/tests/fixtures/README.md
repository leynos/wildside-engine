# OSM PBF test fixtures

The sample OSM extracts used by the unit and behavioural tests are stored as
Base64-encoded blobs to avoid committing binary files.

Decode them when running ingestion manually (outside the test suite):

```bash
base64 --decode wildside-data/tests/fixtures/triangle.osm.pbf.b64 \
  > wildside-data/tests/fixtures/triangle.osm.pbf
base64 --decode wildside-data/tests/fixtures/invalid.osm.pbf.b64 \
  > wildside-data/tests/fixtures/invalid.osm.pbf
base64 --decode wildside-data/tests/fixtures/poi_tags.osm.pbf.b64 \
  > wildside-data/tests/fixtures/poi_tags.osm.pbf
base64 --decode wildside-data/tests/fixtures/irrelevant_tags.osm.pbf.b64 \
  > wildside-data/tests/fixtures/irrelevant_tags.osm.pbf
```

Both the unit and integration tests decode the fixtures into temporary files at
runtime, so running the suite will not leave `.osm.pbf` artefacts in the
repository. The commands above are only required when running the ingestion
code outside the test suite.

- `poi_tags.osm.pbf.b64`: Synthetic Berlin sample combining historic and
  tourism tags (including a dual-tag POI) alongside an irrelevant service way
  for POI extraction tests.
- `irrelevant_tags.osm.pbf.b64`: Dataset with only non-POI tags used to confirm
  filtering skips irrelevant features.
- `invalid_coords.osm.pbf.b64`: Mixed dataset with valid and invalid
  coordinates used to confirm POIs outside the WGS84 bounds are skipped.
