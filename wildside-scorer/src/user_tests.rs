//! Unit coverage for user relevance scoring.

use std::collections::BTreeMap;

use bincode::Options;
use camino::Utf8PathBuf;
use geo::Coord;
use rstest::{fixture, rstest};
use rusqlite::Connection;
use tempfile::TempDir;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, Theme};

use super::{
    ClaimSelector, ScoreWeights, ThemeClaimMapping, UserRelevanceError, UserRelevanceScorer,
};
use crate::{PopularityScores, popularity_bincode_options};

const TEST_PROPERTY: &str = "P999";
const TEST_VALUE: &str = "Q_TEST_ART";

#[rstest]
fn defaults_include_history_mapping() {
    let mapping = ThemeClaimMapping::default();
    assert!(
        mapping.selectors(&Theme::History).is_some(),
        "the default mapping should select claims for the history theme"
    );
}

#[rstest]
fn selector_rejects_empty_fields() {
    let err = ClaimSelector::new("", TEST_VALUE).expect_err("empty property should error");
    assert!(
        matches!(err, UserRelevanceError::InvalidSelector),
        "an empty property should be an InvalidSelector error, got {err:?}"
    );
}

#[rstest]
fn weights_reject_zero_total() {
    let err = ScoreWeights {
        popularity: 0.0,
        user_relevance: 0.0,
    }
    .validate()
    .expect_err("zero weights should be invalid");
    assert!(
        matches!(err, UserRelevanceError::InvalidWeights),
        "zero total weight should be an InvalidWeights error, got {err:?}"
    );
}

#[fixture]
fn seeded_db_path() -> (TempDir, Utf8PathBuf) {
    let temp_dir = TempDir::new().expect("tempdir");
    let path = Utf8PathBuf::from_path_buf(temp_dir.path().join("pois.db")).expect("utf8 db path");
    seed_claims_database(&path);
    (temp_dir, path)
}

#[derive(Clone)]
struct PopularityFixture {
    dir: Utf8PathBuf,
}

impl PopularityFixture {
    fn with_score(&self, poi_id: u64, score: f32) -> Utf8PathBuf {
        let popularity = PopularityScores::new(BTreeMap::from([(poi_id, score)]));
        let path = self.dir.join("popularity.bin");
        let bytes = popularity_bincode_options()
            .serialize(&popularity)
            .expect("serialize popularity");
        wildside_fs::write_file(path.as_std_path(), bytes).expect("write popularity fixture");
        path
    }
}

#[fixture]
fn popularity_fixture() -> (TempDir, PopularityFixture) {
    let temp_dir = TempDir::new().expect("tempdir");
    let dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).expect("utf8 dir path");
    (temp_dir, PopularityFixture { dir })
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "tests compare floating point values"
)]
fn scoring_blends_popularity_and_interest(
    seeded_db_path: (TempDir, Utf8PathBuf),
    popularity_fixture: (TempDir, PopularityFixture),
) {
    let (_pop_temp_dir, pop_fixture) = popularity_fixture;
    let popularity_path = pop_fixture.with_score(1, 0.25_f32);
    let (_db_temp_dir, db_path) = seeded_db_path;

    let mut mapping = ThemeClaimMapping::new();
    mapping.insert(
        Theme::Art,
        ClaimSelector::new(TEST_PROPERTY, TEST_VALUE).expect("valid selector"),
    );
    let scorer = UserRelevanceScorer::from_paths(
        &db_path,
        &popularity_path,
        mapping,
        ScoreWeights::default(),
    )
    .expect("construct scorer");

    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let profile = InterestProfile::new().with_weight(Theme::Art, 0.8_f32);

    let score = scorer.score(&poi, &profile);

    let expected = f32::midpoint(0.25_f32, 0.8_f32);
    assert!(
        (score - expected).abs() < 0.000_1_f32,
        "score should blend components"
    );
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "tests compare floating point values"
)]
fn non_matching_interest_yields_popularity_only(
    seeded_db_path: (TempDir, Utf8PathBuf),
    popularity_fixture: (TempDir, PopularityFixture),
) {
    let (_pop_temp_dir, pop_fixture) = popularity_fixture;
    let popularity_path = pop_fixture.with_score(1, 0.6_f32);
    let (_db_temp_dir, db_path) = seeded_db_path;

    let scorer = UserRelevanceScorer::with_defaults(&db_path, &popularity_path)
        .expect("construct scorer with defaults");
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let profile = InterestProfile::new().with_weight(Theme::Art, 1.0_f32);

    let score = scorer.score(&poi, &profile);

    assert!(
        (score - 0.6_f32).abs() < 0.000_1_f32,
        "non matching interest falls back to popularity"
    );
}

#[rstest]
#[expect(
    clippy::float_arithmetic,
    reason = "tests compare floating point values"
)]
fn missing_popularity_falls_back_to_interest(
    seeded_db_path: (TempDir, Utf8PathBuf),
    popularity_fixture: (TempDir, PopularityFixture),
) {
    let (_pop_temp_dir, pop_fixture) = popularity_fixture;
    let popularity_path = pop_fixture.with_score(2, 0.0_f32);
    let (_db_temp_dir, db_path) = seeded_db_path;

    let mapping = ThemeClaimMapping::default();
    let scorer = UserRelevanceScorer::from_paths(
        &db_path,
        &popularity_path,
        mapping.clone(),
        ScoreWeights {
            popularity: 0.3_f32,
            user_relevance: 0.7_f32,
        },
    )
    .expect("construct scorer");
    let poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    let profile = InterestProfile::new().with_weight(Theme::History, 1.0_f32);

    let score = scorer.score(&poi, &profile);

    assert!(
        (score - 0.7_f32).abs() < 0.000_1_f32,
        "interest match should contribute even without popularity"
    );
}

fn seed_claims_database(path: &Utf8PathBuf) {
    let connection = Connection::open(path.as_std_path()).expect("open sqlite database");
    connection
        .execute(
            concat!(
                "CREATE TABLE poi_wikidata_links (",
                "poi_id INTEGER NOT NULL, ",
                "entity_id TEXT NOT NULL",
                ")"
            ),
            [],
        )
        .expect("create links table");
    connection
        .execute(
            concat!(
                "CREATE TABLE wikidata_entity_claims (",
                "entity_id TEXT NOT NULL, ",
                "property_id TEXT NOT NULL, ",
                "value_entity_id TEXT NOT NULL",
                ")"
            ),
            [],
        )
        .expect("create claims table");
    connection
        .execute(
            concat!(
                "CREATE VIEW poi_wikidata_claims AS ",
                "SELECT links.poi_id AS poi_id, ",
                "claims.entity_id AS entity_id, ",
                "claims.property_id AS property_id, ",
                "claims.value_entity_id AS value_entity_id ",
                "FROM poi_wikidata_links AS links ",
                "JOIN wikidata_entity_claims AS claims ",
                "ON claims.entity_id = links.entity_id"
            ),
            [],
        )
        .expect("create claims view");
    connection
        .execute(
            "INSERT INTO poi_wikidata_links (poi_id, entity_id) VALUES (1, 'Q_ART')",
            [],
        )
        .expect("insert link");
    connection
        .execute(
            "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES ('Q_ART', ?1, ?2)",
            (TEST_PROPERTY, TEST_VALUE),
        )
        .expect("insert claim");
    connection
        .execute(
            "INSERT INTO wikidata_entity_claims (entity_id, property_id, value_entity_id) VALUES ('Q_ART', 'P1435', 'Q9259')",
            [],
        )
        .expect("insert heritage claim");
}

#[cfg(unix)]
#[rstest]
fn from_paths_reads_a_popularity_symlink_that_leads_outside_its_directory(
    seeded_db_path: (TempDir, Utf8PathBuf),
    popularity_fixture: (TempDir, PopularityFixture),
) {
    let (_pop_temp_dir, pop_fixture) = popularity_fixture;
    let real = pop_fixture.with_score(1, 0.25_f32);
    let (_db_temp_dir, db_path) = seeded_db_path;
    let link_dir = TempDir::new().expect("link dir");
    let link =
        Utf8PathBuf::from_path_buf(link_dir.path().join("popularity.bin")).expect("utf8 link");
    std::os::unix::fs::symlink(real.as_std_path(), link.as_std_path()).expect("symlink");

    let built = UserRelevanceScorer::from_paths(
        &db_path,
        &link,
        ThemeClaimMapping::default(),
        ScoreWeights::default(),
    );

    assert!(
        built.is_ok(),
        "a popularity file behind a symlink to another directory should load, got {:?}",
        built.err()
    );
}

#[rstest]
fn from_paths_reads_a_bare_popularity_file_name(
    seeded_db_path: (TempDir, Utf8PathBuf),
    popularity_fixture: (TempDir, PopularityFixture),
) {
    let (_pop_temp_dir, pop_fixture) = popularity_fixture;
    let real = pop_fixture.with_score(1, 0.25_f32);
    let (_db_temp_dir, db_path) = seeded_db_path;
    // A bare name resolves against the current directory, which tests never
    // change; the file is made there and removed after the check.
    let bare = Utf8PathBuf::from(format!("popularity-bare-{}.bin", std::process::id()));
    let bytes = wildside_fs::read_file(real.as_std_path()).expect("read fixture");
    wildside_fs::write_file(bare.as_std_path(), bytes).expect("write bare fixture");

    let built = UserRelevanceScorer::from_paths(
        &db_path,
        &bare,
        ThemeClaimMapping::default(),
        ScoreWeights::default(),
    );
    wildside_fs::remove_file(bare.as_std_path()).expect("remove bare fixture");

    assert!(
        built.is_ok(),
        "a bare popularity file name should load, got {:?}",
        built.err()
    );
}
