use super::*;
use geo::Coord;
use rstest::{fixture, rstest};
use tempfile::TempDir;
use wildside_core::build_spatial_index;

fn write_index(path: &Path, pois: &[PointOfInterest]) {
    let index = build_spatial_index(pois.to_vec());
    let file = File::create(path).expect("create index file");
    let mut writer = std::io::BufWriter::new(file);
    bincode::serialize_into(&mut writer, &index).expect("serialise index");
}

fn write_database(path: &Path, pois: &[PointOfInterest]) {
    let connection = Connection::open(path).expect("open sqlite");
    connection
        .execute(
            "CREATE TABLE pois (id INTEGER PRIMARY KEY, data BLOB NOT NULL)",
            [],
        )
        .expect("create table");
    let mut statement = connection
        .prepare("INSERT INTO pois (id, data) VALUES (?1, ?2)")
        .expect("prepare insert");
    let codec = bincode::DefaultOptions::new();
    for poi in pois {
        let blob = codec.serialize(poi).expect("serialise poi");
        statement.execute((poi.id, blob)).expect("insert poi");
    }
}

#[derive(Debug)]
struct StoreFixture {
    store: SqlitePoiStore,
    _dir: TempDir,
}

#[fixture]
fn sample_pois() -> Vec<PointOfInterest> {
    vec![
        PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 }),
        PointOfInterest::with_empty_tags(2, Coord { x: 1.0, y: 1.0 }),
        PointOfInterest::with_empty_tags(3, Coord { x: -1.0, y: 1.0 }),
    ]
}

#[fixture]
fn store(sample_pois: Vec<PointOfInterest>) -> StoreFixture {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("pois.db");
    let index_path = dir.path().join("pois.rstar");
    write_database(&db_path, &sample_pois);
    write_index(&index_path, &sample_pois);
    let store = SqlitePoiStore::new(&db_path, &index_path).expect("create store");
    StoreFixture { store, _dir: dir }
}

fn bbox(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Rect<f64> {
    Rect::new(Coord { x: min_x, y: min_y }, Coord { x: max_x, y: max_y })
}

#[rstest]
fn returns_pois_inside_bbox(store: StoreFixture) {
    let bbox = bbox(-0.5, -0.5, 0.5, 0.5);
    let pois: Vec<_> = store.store.get_pois_in_bbox(&bbox).collect();
    assert_eq!(pois.len(), 1, "expected a single POI within the bbox");
    assert_eq!(pois[0].id, 1);
}

#[rstest]
fn returns_empty_when_no_matches(store: StoreFixture) {
    let bbox = bbox(5.0, 5.0, 6.0, 6.0);
    let pois: Vec<_> = store.store.get_pois_in_bbox(&bbox).collect();
    assert!(pois.is_empty(), "expected no POIs in distant bbox");
}

#[rstest]
#[case(Coord { x: 0.0, y: 0.0 })]
#[case(Coord { x: 1.0, y: 1.0 })]
#[case(Coord { x: -1.0, y: 1.0 })]
fn includes_boundary_points(store: StoreFixture, #[case] location: Coord<f64>) {
    let bbox = bbox(-1.0, -1.0, 1.0, 1.0);
    let pois: Vec<_> = store.store.get_pois_in_bbox(&bbox).collect();
    assert!(
        pois.iter().any(|poi| poi.location == location),
        "expected POI at {location:?} to be included",
    );
}

#[rstest]
fn creation_fails_when_index_missing(sample_pois: Vec<PointOfInterest>) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("pois.db");
    write_database(&db_path, &sample_pois);
    let index_path = dir.path().join("missing.rstar");
    let err = SqlitePoiStore::new(&db_path, &index_path).expect_err("missing index should fail");
    match err {
        SqlitePoiStoreError::OpenIndex { path, .. } => assert_eq!(path, index_path),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[rstest]
fn creation_fails_when_table_missing() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("pois.db");
    Connection::open(&db_path).expect("open sqlite");
    let index_path = dir.path().join("pois.rstar");
    let empty: Vec<PointOfInterest> = Vec::new();
    write_index(&index_path, &empty);
    let err = SqlitePoiStore::new(&db_path, &index_path).expect_err("missing table should fail");
    match err {
        SqlitePoiStoreError::MissingPoiTable { path } => assert_eq!(path, db_path),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[rstest]
fn creation_fails_when_blob_corrupt(sample_pois: Vec<PointOfInterest>) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("pois.db");
    let connection = Connection::open(&db_path).expect("open sqlite");
    connection
        .execute(
            "CREATE TABLE pois (id INTEGER PRIMARY KEY, data BLOB NOT NULL)",
            [],
        )
        .expect("create table");
    connection
        .execute("INSERT INTO pois (id, data) VALUES (1, x'00')", [])
        .expect("insert corrupt blob");
    let index_path = dir.path().join("pois.rstar");
    write_index(&index_path, &sample_pois);
    let err = SqlitePoiStore::new(&db_path, &index_path).expect_err("corrupt blob should fail");
    match err {
        SqlitePoiStoreError::DecodePoi { id, path, .. } => {
            assert_eq!(id, 1);
            assert_eq!(path, db_path);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[rstest]
fn creation_fails_when_index_references_missing_poi(sample_pois: Vec<PointOfInterest>) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("pois.db");
    write_database(&db_path, &sample_pois[..1]);
    let index_path = dir.path().join("pois.rstar");
    write_index(&index_path, &sample_pois);
    let err = SqlitePoiStore::new(&db_path, &index_path)
        .expect_err("index referencing missing POI should fail");
    match err {
        SqlitePoiStoreError::MissingPois { missing } => {
            assert!(missing.contains(&2));
            assert!(missing.contains(&3));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
