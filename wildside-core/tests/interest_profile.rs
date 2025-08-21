//! Behaviour tests verifying interest profile weight lookups.

use std::collections::HashMap;
use std::str::FromStr;

use rstest::rstest;
use wildside_core::{InterestProfile, Theme};

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
    let map: HashMap<String, f32> = serde_json::from_str(weights).expect("valid weights");
    let mut profile = InterestProfile::new();
    for (k, v) in map {
        profile.set_weight(Theme::from_str(&k).expect("valid theme key"), v);
    }
    let theme = Theme::from_str(theme).expect("valid theme under test");
    assert_eq!(profile.weight(&theme), expected);
}

#[rstest]
#[case(r#"{"history":1.5}"#, "history")]
#[case(r#"{"history":-0.2}"#, "history")]
fn try_set_weight_rejects_out_of_range(#[case] weights: &str, #[case] theme: &str) {
    let map: HashMap<String, f32> = serde_json::from_str(weights).expect("valid weights");
    let mut profile = InterestProfile::new();
    for (k, v) in map {
        assert!(
            profile
                .try_set_weight(Theme::from_str(&k).expect("valid theme key"), v)
                .is_err()
        );
    }
    let theme = Theme::from_str(theme).expect("valid theme under test");
    assert!(profile.weight(&theme).is_none());
}

#[test]
fn invalid_theme_name() {
    assert!(Theme::from_str("sci-fi").is_err());
}
