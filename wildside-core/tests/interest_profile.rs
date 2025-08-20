//! Behaviour-driven tests verifying interest weight lookups for InterestProfile.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use wildside_core::{InterestProfile, Theme};

#[derive(Default)]
struct World {
    profile: Option<InterestProfile>,
    result: Option<f32>,
}

#[fixture]
fn world() -> RefCell<World> {
    RefCell::new(World::default())
}

#[test]
fn query_theme_in_empty_interest_profile() {
    let profile = InterestProfile::new();
    assert!(profile.weight(&Theme::Nature).is_none());
}

#[given("an interest profile with {theme} weight {weight:f32}")]
fn given_profile(#[from(world)] world: &RefCell<World>, theme: Theme, weight: f32) {
    let mut world = world.borrow_mut();
    if let Some(profile) = world.profile.as_mut() {
        profile.set_weight(theme, weight);
    } else {
        world.profile = Some(InterestProfile::new().with_weight(theme, weight));
    }
}

#[given("an empty interest profile")]
fn given_empty_profile(#[from(world)] world: &RefCell<World>) {
    world.borrow_mut().profile = Some(InterestProfile::new());
}

#[when("I query the weight for {theme}")]
fn when_query(#[from(world)] world: &RefCell<World>, theme: Theme) {
    let weight = world
        .borrow()
        .profile
        .as_ref()
        .and_then(|p| p.weight(&theme));
    world.borrow_mut().result = weight;
}

#[then("I get approximately {weight:f32}")]
fn then_result(#[from(world)] world: &RefCell<World>, weight: f32) {
    let actual = world.borrow().result.expect("expected weight");
    assert!(
        (actual - weight).abs() < 1.0e-6,
        "actual={actual}, expected={weight}"
    );
}

#[then("no weight is returned")]
fn then_none(#[from(world)] world: &RefCell<World>) {
    assert!(world.borrow().result.is_none());
}

#[scenario(path = "tests/features/interest_profile.feature", index = 0)]
fn known_theme(world: RefCell<World>) {}

#[scenario(path = "tests/features/interest_profile.feature", index = 1)]
fn unknown_theme(world: RefCell<World>) {}

#[scenario(path = "tests/features/interest_profile.feature", index = 2)]
fn empty_profile(world: RefCell<World>) {}

#[scenario(path = "tests/features/interest_profile.feature", index = 3)]
fn multiple_themes(world: RefCell<World>) {}
