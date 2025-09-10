use geo::Coord;
use rstest::rstest;
use wildside_core::{InterestProfile, PointOfInterest, Scorer, TagScorer, Theme, poi::Tags};

const TOLERANCE: f32 = 1e-6;

#[rstest]
#[case(&["art"], &[(Theme::Art, 0.7)], 0.7)]
#[case(&["history"], &[(Theme::Art, 0.7)], 0.0)]
#[case(&["art", "history"], &[(Theme::Art, 0.7), (Theme::History, 0.2)], 0.9)]
#[case(&["unknown_tag"], &[(Theme::Art, 0.7)], 0.0)]
#[case(&[] as &[&str], &[(Theme::Art, 0.7)], 0.0)]
#[case(&["art"], &[], 0.0)]
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
        profile.set_weight(theme, w);
    }

    let score = TagScorer.score(&poi, &profile);
    assert!((score - expected).abs() <= TOLERANCE);
}
