//! Unit tests verifying interest profile weight lookups and validation.

use std::collections::HashMap;
use std::str::FromStr;

use rstest::rstest;
use wildside_core::{InterestProfile, Theme, profile::WeightError};

#[rstest]
#[case(r#"{"history":0.8}"#, "history", Some(0.8))]
#[case(r#"{"HiStOrY":0.8}"#, "HISTORY", Some(0.8))]
#[case(r#"{"history":0.0}"#, "history", Some(0.0))]
#[case(r#"{"history":1.0}"#, "history", Some(1.0))]
#[case(r#"{"history":0.8}"#, "art", None)]
#[case(r#"{}"#, "history", None)]
#[case(r#"{"history":0.8,"art":0.3}"#, "history", Some(0.8))]
#[case(r#"{"history":0.8,"art":0.3}"#, "art", Some(0.3))]
fn query_weights(#[case] weights: &str, #[case] theme: &str, #[case] expected: Option<f32>) {
    let map: HashMap<String, f32> =
        serde_json::from_str(weights).expect("failed to parse test JSON weights");
    let mut profile = InterestProfile::new();
    for (k, v) in map {
        profile.set_weight(Theme::from_str(&k).expect("valid theme key"), v);
    }
    let theme = Theme::from_str(theme).expect("valid theme under test");
    match (profile.weight(&theme), expected) {
        (Some(actual), Some(expected)) => {
            let eps = 1e-6_f32;
            assert!(
                (actual - expected).abs() <= eps,
                "weight {actual} is not within {eps} of expected {expected}"
            );
        }
        (None, None) => {}
        (got, want) => panic!("weight mismatch: got {got:?}, want {want:?}"),
    }
}

#[rstest]
#[case(r#"{"history":1.5}"#, "history")]
#[case(r#"{"history":-0.2}"#, "history")]
fn try_set_weight_rejects_out_of_range(#[case] weights: &str, #[case] theme: &str) {
    let map: HashMap<String, f32> =
        serde_json::from_str(weights).expect("failed to parse test JSON weights");
    let mut profile = InterestProfile::new();
    for (k, v) in map {
        let err = profile
            .try_set_weight(Theme::from_str(&k).expect("valid theme key"), v)
            .expect_err("expected out-of-range weight to error");
        assert!(
            matches!(err, WeightError::OutOfRange),
            "expected OutOfRange, got {err:?}"
        );
    }
    let theme = Theme::from_str(theme).expect("valid theme under test");
    assert!(profile.weight(&theme).is_none());
}

#[rstest]
#[case("sci-fi")]
#[case("")]
#[case("HISTORY!")]
fn invalid_theme_name(#[case] s: &str) {
    assert!(Theme::from_str(s).is_err());
}
