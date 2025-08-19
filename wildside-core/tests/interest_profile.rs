use std::cell::RefCell;
use std::collections::HashMap;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use wildside_core::InterestProfile;

// Behaviour tests verifying interest weight lookups.

#[derive(Default)]
struct World {
    profile: Option<InterestProfile>,
    result: Option<f32>,
}

#[fixture]
fn world() -> RefCell<World> {
    RefCell::new(World::default())
}

#[given("an interest profile with {theme} weight {weight:f32}")]
fn given_profile(#[from(world)] world: &RefCell<World>, theme: String, weight: f32) {
    let mut weights = HashMap::new();
    weights.insert(theme, weight);
    world.borrow_mut().profile = Some(InterestProfile { weights });
}

#[when("I query the weight for {theme}")]
fn when_query(#[from(world)] world: &RefCell<World>, theme: String) {
    let weight = world
        .borrow()
        .profile
        .as_ref()
        .and_then(|p| p.weights.get(&theme))
        .copied();
    world.borrow_mut().result = weight;
}

#[then("I get {weight:f32}")]
fn then_result(#[from(world)] world: &RefCell<World>, weight: f32) {
    assert_eq!(world.borrow().result, Some(weight));
}

#[then("no weight is returned")]
fn then_none(#[from(world)] world: &RefCell<World>) {
    assert!(world.borrow().result.is_none());
}

#[scenario(path = "tests/features/interest_profile.feature", index = 0)]
fn known_theme(world: RefCell<World>) {}

#[scenario(path = "tests/features/interest_profile.feature", index = 1)]
fn unknown_theme(world: RefCell<World>) {}
