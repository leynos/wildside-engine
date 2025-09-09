use geo::Coord;
use rstest::{fixture, rstest};
use wildside_core::{InterestProfile, PointOfInterest, Scorer, TagScorer, Theme, poi::Tags};

#[fixture]
fn poi_without_tags() -> PointOfInterest {
    PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 })
}

fn poi_with_tags(tags: &[&str]) -> PointOfInterest {
    let tags = tags
        .iter()
        .map(|t| ((*t).to_string(), String::new()))
        .collect::<Tags>();
    PointOfInterest::new(1, Coord { x: 0.0, y: 0.0 }, tags)
}

fn profile_with_weights(weights: &[(Theme, f32)]) -> InterestProfile {
    let mut profile = InterestProfile::new();
    for (theme, weight) in weights {
        profile.set_weight(theme.clone(), *weight);
    }
    profile
}

#[rstest]
fn sums_matching_weights() {
    let poi = poi_with_tags(&["art"]);
    let profile = profile_with_weights(&[(Theme::Art, 0.7)]);
    let scorer = TagScorer;
    assert!((scorer.score(&poi, &profile) - 0.7).abs() <= 1e-6);
}

#[rstest]
fn zero_when_no_match() {
    let poi = poi_with_tags(&["history"]);
    let profile = profile_with_weights(&[(Theme::Art, 0.7)]);
    let scorer = TagScorer;
    assert_eq!(scorer.score(&poi, &profile), 0.0);
}

#[rstest]
fn zero_for_empty_profile() {
    let poi = poi_with_tags(&["art"]);
    let profile = InterestProfile::new();
    let scorer = TagScorer;
    assert_eq!(scorer.score(&poi, &profile), 0.0);
}

#[rstest]
fn sums_multiple_matching_tags() {
    let poi = poi_with_tags(&["art", "history"]);
    let profile = profile_with_weights(&[(Theme::Art, 0.7), (Theme::History, 0.2)]);
    let scorer = TagScorer;
    assert!((scorer.score(&poi, &profile) - 0.9).abs() <= 1e-6);
}

#[rstest]
fn ignores_unknown_tags() {
    let poi = poi_with_tags(&["unknown_tag"]);
    let profile = profile_with_weights(&[(Theme::Art, 0.7)]);
    let scorer = TagScorer;
    assert_eq!(scorer.score(&poi, &profile), 0.0);
}

#[rstest]
fn zero_for_poi_without_tags(poi_without_tags: PointOfInterest) {
    let profile = profile_with_weights(&[(Theme::Art, 0.7)]);
    let scorer = TagScorer;
    assert_eq!(scorer.score(&poi_without_tags, &profile), 0.0);
}
