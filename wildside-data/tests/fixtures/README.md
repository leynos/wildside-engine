# OSM PBF test fixtures

The sample OSM extracts used by the unit and behavioural tests are stored as
Base64-encoded blobs to avoid committing binary files.

Decode them when running ingestion manually (outside the test suite):

```bash
base64 --decode wildside-data/tests/fixtures/triangle.osm.pbf.b64 \
  > wildside-data/tests/fixtures/triangle.osm.pbf
base64 --decode wildside-data/tests/fixtures/invalid.osm.pbf.b64 \
  > wildside-data/tests/fixtures/invalid.osm.pbf
```

Both the unit and integration tests decode the fixtures into temporary files at
runtime, so running the suite will not leave `.osm.pbf` artefacts in the
repository. The commands above are only required when running the ingestion
code outside the test suite.
