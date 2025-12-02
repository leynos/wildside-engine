#![expect(
    clippy::expect_used,
    reason = "tests should fail fast when setup breaks"
)]

//! Behavioural coverage for user relevance scoring.

use std::cell::RefCell;

use bincode::Options;
use camino::Utf8PathBuf;
use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, Theme};
use wildside_scorer::{
    ClaimSelector, ScoreWeights, ThemeClaimMapping, UserRelevanceScorer, popularity_bincode_options,
};

const ART_PROPERTY: &str = "P999";
const ART_VALUE: &str = "Q_ART";

/// Aggregate fixtures shared across the BDD scenarios.
pub struct TestContext {
    temp_dir: TempDir,
    db_path: RefCell<Option<Utf8PathBuf>>,
    popularity_path: RefCell<Option<Utf8PathBuf>>,
    mapping: ThemeClaimMapping,
    scored_value: RefCell<Option<f32>>,
}

#[fixture]
/// Build a fresh `TestContext` for each scenario run.
pub fn context() -> TestContext {
    let mut mapping = ThemeClaimMapping::default();
    let selector = ClaimSelector::new(ART_PROPERTY, ART_VALUE).expect("valid art selector");
    mapping.insert(Theme::Art, selector);

    TestContext {
        temp_dir: TempDir::new().expect("create tempdir for scenario"),
        db_path: RefCell::new(None),
        popularity_path: RefCell::new(None),
        mapping,
        scored_value: RefCell::new(None),
    }
}

#[given("a SQLite POI database with themed claims")]
#[expect(clippy::expect_used, reason = "fixtures should fail fast during setup")]
fn sqlite_with_claims(context: &TestContext) {
    let path = Utf8PathBuf::from_path_buf(context.temp_dir.path().join("pois.db"))
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

    *context.db_path.borrow_mut() = Some(path);
}

#[given("a popularity file where the POI scores 0.3")]
fn popularity_score_low(context: &TestContext) {
    write_popularity_file(context, 0.3_f32);
}

#[given("a popularity file where the POI scores 0.7")]
fn popularity_score_high(context: &TestContext) {
    write_popularity_file(context, 0.7_f32);
}

#[given("a popularity file without an entry for the POI")]
fn popularity_without_entry(context: &TestContext) {
    write_popularity_scores(
        context,
        std::collections::BTreeMap::from([(99_u64, 0.4_f32)]),
    );
}

#[when("I score the POI for an art-loving visitor")]
fn score_for_art(context: &TestContext) {
    score_poi_with_theme(context, Theme::Art, 0.9_f32);
}

#[when("I score the POI for a food-loving visitor")]
fn score_for_food(context: &TestContext) {
    score_poi_with_theme(context, Theme::Food, 0.8_f32);
}

#[when("I score the POI for a history-loving visitor")]
fn score_for_history(context: &TestContext) {
    score_poi_with_theme(context, Theme::History, 1.0_f32);
}

#[then("the score combines popularity with the art interest")]
fn assert_art_score(context: &TestContext) {
    assert_score_near(context, 0.6_f32, "expected blended score of 0.6");
}

#[then("the score equals the popularity component")]
fn assert_food_score(context: &TestContext) {
    assert_score_near(context, 0.7_f32, "expected popularity-only score");
}

#[then("the score is driven by the history interest")]
fn assert_history_score(context: &TestContext) {
    assert_score_near(context, 0.5_f32, "expected interest-led score");
}

fn write_popularity_file(context: &TestContext, score: f32) {
    write_popularity_scores(context, std::collections::BTreeMap::from([(1_u64, score)]));
}

fn build_scorer(context: &TestContext) -> UserRelevanceScorer {
    let db = context
        .db_path
        .borrow()
        .as_ref()
        .cloned()
        .expect("database path must be initialised");
    let popularity = context
        .popularity_path
        .borrow()
        .as_ref()
        .cloned()
        .expect("popularity path must be initialised");
    UserRelevanceScorer::from_paths(
        &db,
        &popularity,
        context.mapping.clone(),
        ScoreWeights::default(),
    )
    .expect("construct scorer")
}

fn record_score(cell: &RefCell<Option<f32>>, score: f32) {
    *cell.borrow_mut() = Some(score);
}

fn write_popularity_scores(
    context: &TestContext,
    scores_map: std::collections::BTreeMap<u64, f32>,
) {
    use std::collections::BTreeMap;

    let path = Utf8PathBuf::from_path_buf(context.temp_dir.path().join("popularity.bin"))
        .expect("utf8 popularity path");
    let scores = wildside_scorer::PopularityScores::new(BTreeMap::from_iter(scores_map));
    let bytes = popularity_bincode_options()
        .serialize(&scores)
        .expect("serialise popularity scores");
    std::fs::write(path.as_std_path(), bytes).expect("write popularity file");
    *context.popularity_path.borrow_mut() = Some(path);
}

fn score_poi_with_theme(context: &TestContext, theme: Theme, weight: f32) {
    let scorer = build_scorer(context);
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let profile = InterestProfile::new().with_weight(theme, weight);
    record_score(&context.scored_value, scorer.score(&poi, &profile));
}

#[expect(
    clippy::float_arithmetic,
    reason = "assertions compare floating point values"
)]
fn assert_score_near(context: &TestContext, expected: f32, message: &str) {
    let score = context
        .scored_value
        .borrow()
        .expect("score should be recorded");
    assert!((score - expected).abs() < 0.000_1_f32, "{message}");
}

#[scenario(path = "tests/features/user_relevance.feature", index = 0)]
fn art_interest_blends_popularity(context: TestContext) {
    let _ = context;
}

#[scenario(path = "tests/features/user_relevance.feature", index = 1)]
fn unmatched_interest_falls_back_to_popularity(context: TestContext) {
    let _ = context;
}

#[scenario(path = "tests/features/user_relevance.feature", index = 2)]
fn missing_popularity_relies_on_interest(context: TestContext) {
    let _ = context;
}
