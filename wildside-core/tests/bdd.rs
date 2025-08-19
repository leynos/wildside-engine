use std::collections::HashMap;
use std::time::Duration;

use cucumber::{World, given, then, when, writer::Basic};
use geo::Coord;
use wildside_core::{
    InterestProfile, InterestProfileError, PointOfInterest, PointOfInterestError, Route, RouteError,
};

#[derive(Debug, Default, World)]
struct DomainWorld {
    poi_result: Option<Result<PointOfInterest, PointOfInterestError>>,
    profile_result: Option<Result<InterestProfile, InterestProfileError>>,
    route_result: Option<Result<Route, RouteError>>,
    poi: Option<PointOfInterest>,
}

#[when(regex = r"^I create a point of interest with id (\\d+) and tag (\\w+)=(\\w+)$")]
fn create_poi(world: &mut DomainWorld, id: u64, key: String, value: String) {
    let mut tags = HashMap::new();
    tags.insert(key, value);
    world.poi_result = Some(PointOfInterest::new(id, Coord { x: 0.0, y: 0.0 }, tags));
}

#[when(regex = r"^I create a point of interest with id (\\d+) and no tags$")]
fn create_poi_no_tags(world: &mut DomainWorld, id: u64) {
    world.poi_result = Some(PointOfInterest::new(
        id,
        Coord { x: 0.0, y: 0.0 },
        HashMap::new(),
    ));
}

#[then("the point of interest is created")]
fn poi_created(world: &mut DomainWorld) {
    assert!(matches!(world.poi_result, Some(Ok(_))));
}

#[then("a point of interest error is returned")]
fn poi_error(world: &mut DomainWorld) {
    assert!(matches!(world.poi_result, Some(Err(_))));
}

#[when(regex = r"^I create an interest profile with theme (\\w+) and weight ([0-9.]+)$")]
fn create_profile(world: &mut DomainWorld, theme: String, weight: f32) {
    let mut weights = HashMap::new();
    weights.insert(theme, weight);
    world.profile_result = Some(InterestProfile::new(weights));
}

#[then("the interest profile is created")]
fn profile_created(world: &mut DomainWorld) {
    assert!(matches!(world.profile_result, Some(Ok(_))));
}

#[then("an interest profile error is returned")]
fn profile_error(world: &mut DomainWorld) {
    assert!(matches!(world.profile_result, Some(Err(_))));
}

#[given(regex = r"^a point of interest with id (\\d+) and tag (\\w+)=(\\w+)$")]
fn given_poi(world: &mut DomainWorld, id: u64, key: String, value: String) {
    let mut tags = HashMap::new();
    tags.insert(key, value);
    world.poi = Some(PointOfInterest::new(id, Coord { x: 0.0, y: 0.0 }, tags).unwrap());
}

#[when(regex = r"^I create a route with that point and duration (\\d+)$")]
fn create_route(world: &mut DomainWorld, minutes: u64) {
    let poi = world.poi.clone().into_iter().collect();
    world.route_result = Some(Route::new(poi, Duration::from_secs(minutes * 60)));
}

#[when(regex = r"^I create a route with no points and duration (\\d+)$")]
fn create_route_no_points(world: &mut DomainWorld, minutes: u64) {
    world.route_result = Some(Route::new(Vec::new(), Duration::from_secs(minutes * 60)));
}

#[then("the route is created")]
fn route_created(world: &mut DomainWorld) {
    assert!(matches!(world.route_result, Some(Ok(_))));
}

#[then("a route error is returned")]
fn route_error(world: &mut DomainWorld) {
    assert!(matches!(world.route_result, Some(Err(_))));
}

#[tokio::main]
async fn main() {
    DomainWorld::cucumber()
        .with_writer(Basic::stdout())
        .run("features")
        .await;
}
