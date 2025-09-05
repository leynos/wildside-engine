//! Behavioural tests for `Scorer` implementations.

use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use std::str::FromStr;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, Theme, poi::Tags};

struct TagScorer;

impl Scorer for TagScorer {
    fn score(&self, poi: &PointOfInterest, profile: &InterestProfile) -> f32 {
        poi.tags
            .keys()
            .filter_map(|k| Theme::from_str(k).ok())
            .filter_map(|t| profile.weight(&t))
            .sum()
    }
}

#[fixture]
fn scorer() -> TagScorer {
    TagScorer
}

#[fixture]
fn poi() -> RefCell<PointOfInterest> {
    RefCell::new(PointOfInterest::with_empty_tags(
        1,
        Coord { x: 0.0, y: 0.0 },
    ))
}

#[fixture]
fn profile() -> RefCell<InterestProfile> {
    RefCell::new(InterestProfile::new())
}

#[fixture]
fn result() -> Cell<f32> {
    Cell::new(0.0)
}

#[given("a POI tagged 'art' and a profile with 'art' weight 0.7")]
fn given_matching(
    #[from(poi)] poi: &RefCell<PointOfInterest>,
    #[from(profile)] profile: &RefCell<InterestProfile>,
) {
    poi.borrow_mut().tags = Tags::from([("art".into(), String::new())]);
    profile.borrow_mut().set_weight(Theme::Art, 0.7);
}

#[given("a POI tagged 'history' and a profile with 'art' weight 0.7")]
fn given_non_matching(
    #[from(poi)] poi: &RefCell<PointOfInterest>,
    #[from(profile)] profile: &RefCell<InterestProfile>,
) {
    poi.borrow_mut().tags = Tags::from([("history".into(), String::new())]);
    profile.borrow_mut().set_weight(Theme::Art, 0.7);
}

#[when("I score the POI")]
fn when_score(
    #[from(scorer)] scorer: &TagScorer,
    #[from(poi)] poi: &RefCell<PointOfInterest>,
    #[from(profile)] profile: &RefCell<InterestProfile>,
    #[from(result)] result: &Cell<f32>,
) {
    result.set(scorer.score(&poi.borrow(), &profile.borrow()));
}

#[then("the score is 0.7")]
fn then_score_0_7(#[from(result)] result: &Cell<f32>) {
    assert!((result.get() - 0.7).abs() < f32::EPSILON);
}

#[then("the score is 0.0")]
fn then_score_0(#[from(result)] result: &Cell<f32>) {
    assert_eq!(result.get(), 0.0);
}

#[scenario(path = "tests/features/scorer.feature", index = 0)]
fn match_tag(
    scorer: TagScorer,
    poi: RefCell<PointOfInterest>,
    profile: RefCell<InterestProfile>,
    result: Cell<f32>,
) {
    let _ = (scorer, poi, profile, result);
}

#[scenario(path = "tests/features/scorer.feature", index = 1)]
fn miss_tag(
    scorer: TagScorer,
    poi: RefCell<PointOfInterest>,
    profile: RefCell<InterestProfile>,
    result: Cell<f32>,
) {
    let _ = (scorer, poi, profile, result);
}
