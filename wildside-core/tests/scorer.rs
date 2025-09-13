use geo::Coord;
use rstest::rstest;
use wildside_core::profile::test_support::InterestProfileTestExt;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, TagScorer, Theme, poi::Tags};

const TOLERANCE: f32 = 1e-6;

#[rstest]
#[case(&["art"], &[(Theme::Art, 0.7)], 0.7)]
#[case(&["history"], &[(Theme::Art, 0.7)], 0.0)]
#[case(&["art", "history"], &[(Theme::Art, 0.7), (Theme::History, 0.2)], 0.9)]
// Duplicate tags should not count weights multiple times
#[case(&["art", "art"], &[(Theme::Art, 0.7)], 0.7)]
#[case(&["unknown_tag"], &[(Theme::Art, 0.7)], 0.0)]
#[case(&[] as &[&str], &[(Theme::Art, 0.7)], 0.0)]
#[case(&["art"], &[], 0.0)]
// Sum > 1.0 should clamp to 1.0
#[case(&["art", "history"], &[(Theme::Art, 0.8), (Theme::History, 0.5)], 1.0)]
// Extremely large weights should clamp to 1.0
#[case(&["art"], &[(Theme::Art, f32::MAX)], 1.0)]
// Negative weights should not produce negative scores
#[case(&["art"], &[(Theme::Art, -0.2)], 0.0)]
// Non-finite weights should yield 0.0
#[case(&["art"], &[(Theme::Art, f32::INFINITY)], 0.0)]
#[case(&["art"], &[(Theme::Art, f32::NAN)], 0.0)]
fn score_tag_scenarios(
    #[case] tags: &[&str],
    #[case] weights: &[(Theme, f32)],
    #[case] expected: f32,
) {
    let mut poi = PointOfInterest::with_empty_tags(1, Coord { x: 0.0, y: 0.0 });
    poi.tags = tags
        .iter()
        .map(|&t| (t.into(), String::new()))
        .collect::<Tags>();

    let mut profile = InterestProfile::new();
    for (theme, w) in weights.iter().cloned() {
        profile.insert_raw_weight(theme, w);
    }

    let score = TagScorer.score(&poi, &profile);
    assert!(score.is_finite(), "score must be finite");
    assert!(
        (-TOLERANCE..=1.0 + TOLERANCE).contains(&score),
        "score must be within [0, 1]"
    );
    assert!((score - expected).abs() <= TOLERANCE);
}

#[rstest]
#[case(f32::NAN, 0.0)]
#[case(f32::INFINITY, 0.0)]
#[case(f32::NEG_INFINITY, 0.0)]
#[case(-0.1, 0.0)]
#[case(1.2, 1.0)]
#[case(0.4, 0.4)]
fn sanitise_clamps_and_filters(#[case] input: f32, #[case] expected: f32) {
    let result = TagScorer::sanitise(input);
    assert!(result.is_finite(), "result must be finite");
    assert!(
        (0.0..=1.0).contains(&result),
        "result must be within [0, 1]"
    );
    assert!((result - expected).abs() <= TOLERANCE);
}
