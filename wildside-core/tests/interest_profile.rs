//! Behaviour tests verifying interest profile weight lookups.

use std::collections::HashMap;
use std::str::FromStr;

use rstest::rstest;
use wildside_core::{InterestProfile, Theme};

#[rstest]
#[case(r#"{"history":0.8}"#, "history", Some(0.8))]
#[case(r#"{"history":0.8}"#, "art", None)]
#[case(r#"{}"#, "history", None)]
#[case(r#"{"history":0.8,"art":0.3}"#, "history", Some(0.8))]
#[case(r#"{"history":0.8,"art":0.3}"#, "art", Some(0.3))]
fn query_weights(#[case] weights: &str, #[case] theme: &str, #[case] expected: Option<f32>) {
    let map: HashMap<String, f32> = serde_json::from_str(weights).expect("valid weights");
    let mut profile = InterestProfile::new();
    for (k, v) in map {
        profile.set_weight(Theme::from_str(&k).unwrap(), v);
    }
    let theme = Theme::from_str(theme).unwrap();
    assert_eq!(profile.weight(&theme), expected);
}

#[test]
fn invalid_theme_name() {
    assert!(Theme::from_str("sci-fi").is_err());
}
