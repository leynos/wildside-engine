use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use wildside_core::{InterestProfile, PointOfInterest, Scorer, TagScorer, Theme, poi::Tags};

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

#[given(
    "a POI tagged 'art' and 'history' and a profile with 'art' weight 0.7 and 'history' weight 0.2"
)]
fn given_multiple(
    #[from(poi)] poi: &RefCell<PointOfInterest>,
    #[from(profile)] profile: &RefCell<InterestProfile>,
) {
    poi.borrow_mut().tags = Tags::from([
        ("art".into(), String::new()),
        ("history".into(), String::new()),
    ]);
    let mut profile = profile.borrow_mut();
    profile.set_weight(Theme::Art, 0.7);
    profile.set_weight(Theme::History, 0.2);
}

#[given("a POI tagged 'unknown_tag' and a profile with 'art' weight 0.7")]
fn given_unknown(
    #[from(poi)] poi: &RefCell<PointOfInterest>,
    #[from(profile)] profile: &RefCell<InterestProfile>,
) {
    poi.borrow_mut().tags = Tags::from([("unknown_tag".into(), String::new())]);
    profile.borrow_mut().set_weight(Theme::Art, 0.7);
}

#[given("a POI with no tags and a profile with 'art' weight 0.7")]
fn given_no_tags(
    #[from(poi)] poi: &RefCell<PointOfInterest>,
    #[from(profile)] profile: &RefCell<InterestProfile>,
) {
    poi.borrow_mut().tags = Tags::new();
    profile.borrow_mut().set_weight(Theme::Art, 0.7);
}

#[when("I score the POI")]
fn when_score(
    #[from(scorer)] scorer: TagScorer,
    #[from(poi)] poi: &RefCell<PointOfInterest>,
    #[from(profile)] profile: &RefCell<InterestProfile>,
    #[from(result)] result: &Cell<f32>,
) {
    let poi = poi.borrow();
    let profile = profile.borrow();
    result.set(scorer.score(&poi, &profile));
}

#[then("the result is {float}")]
fn then_result(expected: f32, #[from(result)] result: &Cell<f32>) {
    assert!((result.get() - expected).abs() <= 1e-6);
}

#[scenario(path = "tests/features/scorer.feature", index = 0)]
fn matching_tag(
    scorer: TagScorer,
    poi: RefCell<PointOfInterest>,
    profile: RefCell<InterestProfile>,
    result: Cell<f32>,
) {
    let _ = (scorer, poi, profile, result);
}

#[scenario(path = "tests/features/scorer.feature", index = 1)]
fn non_matching_tag(
    scorer: TagScorer,
    poi: RefCell<PointOfInterest>,
    profile: RefCell<InterestProfile>,
    result: Cell<f32>,
) {
    let _ = (scorer, poi, profile, result);
}

#[scenario(path = "tests/features/scorer.feature", index = 2)]
fn multiple_tags(
    scorer: TagScorer,
    poi: RefCell<PointOfInterest>,
    profile: RefCell<InterestProfile>,
    result: Cell<f32>,
) {
    let _ = (scorer, poi, profile, result);
}

#[scenario(path = "tests/features/scorer.feature", index = 3)]
fn unknown_tag(
    scorer: TagScorer,
    poi: RefCell<PointOfInterest>,
    profile: RefCell<InterestProfile>,
    result: Cell<f32>,
) {
    let _ = (scorer, poi, profile, result);
}

#[scenario(path = "tests/features/scorer.feature", index = 4)]
fn no_tags(
    scorer: TagScorer,
    poi: RefCell<PointOfInterest>,
    profile: RefCell<InterestProfile>,
    result: Cell<f32>,
) {
    let _ = (scorer, poi, profile, result);
}
