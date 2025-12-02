#![allow(missing_docs, reason = "integration test fixtures keep boilerplate low")]
#![allow(clippy::expect_used, reason = "tests should fail fast when setup breaks")]

//! Behavioural coverage for user relevance scoring.

use std::cell::RefCell;

use bincode::Options;
use camino::Utf8PathBuf;
use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, Theme};
use wildside_scorer::{ClaimSelector, ScoreWeights, ThemeClaimMapping, UserRelevanceScorer};

const ART_PROPERTY: &str = "P999";
const ART_VALUE: &str = "Q_ART";

#[fixture]
pub fn temp_dir() -> TempDir {
    TempDir::new().expect("create tempdir for scenario")
}

#[fixture]
pub fn db_path() -> RefCell<Option<Utf8PathBuf>> {
    RefCell::new(None)
}

#[fixture]
pub fn popularity_path() -> RefCell<Option<Utf8PathBuf>> {
    RefCell::new(None)
}

#[fixture]
pub fn mapping() -> ThemeClaimMapping {
    let mut mapping = ThemeClaimMapping::default();
    let selector = ClaimSelector::new(ART_PROPERTY, ART_VALUE).expect("valid art selector");
    mapping.insert(Theme::Art, selector);
    mapping
}

#[fixture]
pub fn scored_value() -> RefCell<Option<f32>> {
    RefCell::new(None)
}

#[given("a SQLite POI database with themed claims")]
#[expect(clippy::expect_used, reason = "fixtures should fail fast during setup")]
fn sqlite_with_claims(temp_dir: &TempDir, db_path: &RefCell<Option<Utf8PathBuf>>) {
    let path = Utf8PathBuf::from_path_buf(temp_dir.path().join("pois.db"))
        .expect("utf8 path for database");
    let connection =
        rusqlite::Connection::open(path.as_std_path()).expect("open sqlite database for claims");
    connection
        .execute(
            "CREATE TABLE poi_wikidata_links (poi_id INTEGER NOT NULL, entity_id TEXT NOT NULL)",
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
            "CREATE VIEW poi_wikidata_claims AS
                SELECT
                    links.poi_id AS poi_id,
                    claims.entity_id AS entity_id,
                    claims.property_id AS property_id,
                    claims.value_entity_id AS value_entity_id
                FROM poi_wikidata_links AS links
                JOIN wikidata_entity_claims AS claims
                    ON claims.entity_id = links.entity_id",
            [],
        )
        .expect("create claims view");
    connection
        .execute(
            "INSERT INTO poi_wikidata_links (poi_id, entity_id) VALUES (1, 'Q_ART')",
            [],
        )
        .expect("link poi to entity");
    connection
        .execute(
            "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES
                ('Q_ART', ?1, ?2)",
            (ART_PROPERTY, ART_VALUE),
        )
        .expect("insert art claim");
    connection
        .execute(
            "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES
                ('Q_ART', 'P1435', 'Q9259')",
            [],
        )
        .expect("insert heritage claim");

    *db_path.borrow_mut() = Some(path);
}

#[given("a popularity file where the POI scores 0.3")]
fn popularity_score_low(temp_dir: &TempDir, popularity_path: &RefCell<Option<Utf8PathBuf>>) {
    write_popularity_file(temp_dir, popularity_path, 0.3_f32);
}

#[given("a popularity file where the POI scores 0.7")]
fn popularity_score_high(temp_dir: &TempDir, popularity_path: &RefCell<Option<Utf8PathBuf>>) {
    write_popularity_file(temp_dir, popularity_path, 0.7_f32);
}

#[given("a popularity file without an entry for the POI")]
fn popularity_without_entry(temp_dir: &TempDir, popularity_path: &RefCell<Option<Utf8PathBuf>>) {
    use std::collections::BTreeMap;

    let path = Utf8PathBuf::from_path_buf(temp_dir.path().join("popularity.bin"))
        .expect("utf8 popularity path");
    let scores = wildside_scorer::PopularityScores::new(BTreeMap::from([(99_u64, 0.4_f32)]));
    let bytes = bincode::DefaultOptions::new()
        .serialize(&scores)
        .expect("serialise popularity without entry");
    std::fs::write(path.as_std_path(), bytes).expect("write popularity without entry");
    *popularity_path.borrow_mut() = Some(path);
}

#[when("I score the POI for an art-loving visitor")]
fn score_for_art(
    db_path: &RefCell<Option<Utf8PathBuf>>,
    popularity_path: &RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
    scored_value: &RefCell<Option<f32>>,
) {
    let scorer = build_scorer(db_path, popularity_path, mapping);
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let profile = InterestProfile::new().with_weight(Theme::Art, 0.9_f32);
    record_score(scored_value, scorer.score(&poi, &profile));
}

#[when("I score the POI for a food-loving visitor")]
fn score_for_food(
    db_path: &RefCell<Option<Utf8PathBuf>>,
    popularity_path: &RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
    scored_value: &RefCell<Option<f32>>,
) {
    let scorer = build_scorer(db_path, popularity_path, mapping);
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let profile = InterestProfile::new().with_weight(Theme::Food, 0.8_f32);
    record_score(scored_value, scorer.score(&poi, &profile));
}

#[when("I score the POI for a history-loving visitor")]
fn score_for_history(
    db_path: &RefCell<Option<Utf8PathBuf>>,
    popularity_path: &RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
    scored_value: &RefCell<Option<f32>>,
) {
    let scorer = build_scorer(db_path, popularity_path, mapping);
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let profile = InterestProfile::new().with_weight(Theme::History, 1.0_f32);
    record_score(scored_value, scorer.score(&poi, &profile));
}

#[then("the score combines popularity with the art interest")]
#[expect(
    clippy::float_arithmetic,
    reason = "assertions compare floating point values"
)]
fn assert_art_score(scored_value: &RefCell<Option<f32>>) {
    let score = scored_value.borrow().expect("score should be recorded");
    assert!(
        (score - 0.6_f32).abs() < 0.000_1_f32,
        "expected blended score of 0.6"
    );
}

#[then("the score equals the popularity component")]
#[expect(
    clippy::float_arithmetic,
    reason = "assertions compare floating point values"
)]
fn assert_food_score(scored_value: &RefCell<Option<f32>>) {
    let score = scored_value.borrow().expect("score should be recorded");
    assert!(
        (score - 0.7_f32).abs() < 0.000_1_f32,
        "expected popularity-only score"
    );
}

#[then("the score is driven by the history interest")]
#[expect(
    clippy::float_arithmetic,
    reason = "assertions compare floating point values"
)]
fn assert_history_score(scored_value: &RefCell<Option<f32>>) {
    let score = scored_value.borrow().expect("score should be recorded");
    assert!(
        (score - 0.5_f32).abs() < 0.000_1_f32,
        "expected interest-led score"
    );
}

fn write_popularity_file(
    temp_dir: &TempDir,
    popularity_path: &RefCell<Option<Utf8PathBuf>>,
    score: f32,
) {
    use std::collections::BTreeMap;

    let path = Utf8PathBuf::from_path_buf(temp_dir.path().join("popularity.bin"))
        .expect("utf8 popularity path");
    let scores = wildside_scorer::PopularityScores::new(BTreeMap::from([(1_u64, score)]));
    let bytes = bincode::DefaultOptions::new()
        .serialize(&scores)
        .expect("serialise popularity scores");
    std::fs::write(path.as_std_path(), bytes).expect("write popularity file");
    *popularity_path.borrow_mut() = Some(path);
}

fn build_scorer(
    db_path: &RefCell<Option<Utf8PathBuf>>,
    popularity_path: &RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
) -> UserRelevanceScorer {
    let db = db_path
        .borrow()
        .as_ref()
        .cloned()
        .expect("database path must be initialised");
    let popularity = popularity_path
        .borrow()
        .as_ref()
        .cloned()
        .expect("popularity path must be initialised");
    UserRelevanceScorer::from_paths(&db, &popularity, mapping, ScoreWeights::default())
        .expect("construct scorer")
}

fn record_score(cell: &RefCell<Option<f32>>, score: f32) {
    *cell.borrow_mut() = Some(score);
}

#[scenario(path = "tests/features/user_relevance.feature", index = 0)]
#[expect(
    clippy::too_many_arguments,
    reason = "fixtures are dictated by rstest-bdd"
)]
fn art_interest_blends_popularity(
    temp_dir: TempDir,
    db_path: RefCell<Option<Utf8PathBuf>>,
    popularity_path: RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
    scored_value: RefCell<Option<f32>>,
) {
    let _ = (temp_dir, db_path, popularity_path, mapping, scored_value);
}

#[scenario(path = "tests/features/user_relevance.feature", index = 1)]
#[expect(
    clippy::too_many_arguments,
    reason = "fixtures are dictated by rstest-bdd"
)]
fn unmatched_interest_falls_back_to_popularity(
    temp_dir: TempDir,
    db_path: RefCell<Option<Utf8PathBuf>>,
    popularity_path: RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
    scored_value: RefCell<Option<f32>>,
) {
    let _ = (temp_dir, db_path, popularity_path, mapping, scored_value);
}

#[scenario(path = "tests/features/user_relevance.feature", index = 2)]
#[expect(
    clippy::too_many_arguments,
    reason = "fixtures are dictated by rstest-bdd"
)]
fn missing_popularity_relies_on_interest(
    temp_dir: TempDir,
    db_path: RefCell<Option<Utf8PathBuf>>,
    popularity_path: RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
    scored_value: RefCell<Option<f32>>,
) {
    let _ = (temp_dir, db_path, popularity_path, mapping, scored_value);
}
