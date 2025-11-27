//! Unit coverage for popularity scoring helpers.
#![forbid(unsafe_code)]

use camino::Utf8PathBuf;
use rstest::rstest;
use rusqlite::Connection;
use tempfile::TempDir;

use crate::{normalise_scores, resolver::SitelinkResolver, resolver::parse_sitelinks_from_tags};

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "test uses float maths for assertions"
)]
fn normalises_scores() {
    let mut raw = std::collections::HashMap::new();
    raw.insert(1, 10.0_f32);
    raw.insert(2, 5.0_f32);

    let normalised = normalise_scores(&raw);

    assert_eq!(normalised.get(&1), Some(&1.0_f32));
    let value = normalised.get(&2).expect("score for poi 2");
    let delta = (value - 0.5_f32).abs();
    assert!(
        delta < 0.000_1_f32,
        "expected approximately 0.5, got {value}"
    );
}

#[rstest]
fn normalises_zero_scores_to_zero() {
    let mut raw = std::collections::HashMap::new();
    raw.insert(1, 0.0_f32);
    raw.insert(2, 0.0_f32);

    let normalised = normalise_scores(&raw);

    assert_eq!(normalised.get(&1), Some(&0.0_f32));
    assert_eq!(normalised.get(&2), Some(&0.0_f32));
}

#[rstest]
fn parses_numeric_sitelinks_from_tags() {
    let tags = r#"{"wikidata":"Q64","sitelinks":42}"#;

    let parsed = parse_sitelinks_from_tags(tags, 1).expect("parse sitelinks");

    assert_eq!(parsed, Some(42));
}

#[rstest]
fn parses_string_sitelinks_from_tags() {
    let tags = r#"{"wikidata":"Q64","sitelinks":"17"}"#;

    let parsed = parse_sitelinks_from_tags(tags, 1).expect("parse sitelinks");

    assert_eq!(parsed, Some(17));
}

#[rstest]
fn sitelink_table_is_preferred() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = Utf8PathBuf::from_path_buf(temp.path().join("pois.db")).expect("utf8 path");
    seed_database(&db_path);
    let connection = Connection::open(db_path.as_std_path()).expect("open database with sitelinks");
    connection
        .execute(
            "CREATE TABLE wikidata_entity_sitelinks (entity_id TEXT PRIMARY KEY, sitelink_count INTEGER NOT NULL)",
            [],
        )
        .expect("create sitelink table");
    connection
        .execute(
            "INSERT INTO wikidata_entity_sitelinks (entity_id, sitelink_count) VALUES (?1, ?2)",
            ("Q64", 99_i64),
        )
        .expect("insert sitelink count");

    let mut resolver = SitelinkResolver::new(&connection).expect("create resolver");
    let count = resolver
        .sitelink_count(Some("Q64"), r#"{"wikidata":"Q64"}"#, 1)
        .expect("resolve sitelinks");

    assert_eq!(count, 99);
}

fn seed_database(path: &Utf8PathBuf) {
    let connection = Connection::open(path.as_std_path()).expect("open database");
    connection
        .execute(
            "CREATE TABLE pois (
                id INTEGER PRIMARY KEY,
                lon REAL NOT NULL,
                lat REAL NOT NULL,
                tags TEXT NOT NULL
            )",
            [],
        )
        .expect("create pois table");
    connection
        .execute(
            "CREATE TABLE poi_wikidata_links (
                poi_id INTEGER NOT NULL,
                entity_id TEXT NOT NULL,
                PRIMARY KEY (poi_id, entity_id)
            )",
            [],
        )
        .expect("create links table");
    connection
        .execute(
            "CREATE TABLE wikidata_entity_claims (
                entity_id TEXT NOT NULL,
                property_id TEXT NOT NULL,
                value_entity_id TEXT NOT NULL
            )",
            [],
        )
        .expect("create claims table");
    connection
        .execute(
            "INSERT INTO pois (id, lon, lat, tags) VALUES (1, 0.0, 0.0, '{\"wikidata\":\"Q64\"}')",
            [],
        )
        .expect("insert poi");
    connection
        .execute(
            "INSERT INTO poi_wikidata_links (poi_id, entity_id) VALUES (1, 'Q64')",
            [],
        )
        .expect("link poi to entity");
    connection
        .execute(
            "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES ('Q64', 'P1435', 'Q9259')",
            [],
        )
        .expect("insert heritage claim");
}
