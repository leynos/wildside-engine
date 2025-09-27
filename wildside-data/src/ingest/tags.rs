//! Tag utilities for POI extraction.
//!
//! Provides helpers to:
//! - detect whether an element carries POI-marker tags (historic, tourism); and
//! - collect key/value tags into the POI tag map.
use wildside_core::poi::Tags as PoiTags;

pub(super) fn has_relevant_key<'a, T>(tags: T) -> bool
where
    T: IntoIterator<Item = (&'a str, &'a str)>,
{
    tags.into_iter().any(|(key, _)| is_relevant_key(key))
}

pub(super) fn collect_tags<'a, T>(tags: T) -> PoiTags
where
    T: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut collected = PoiTags::new();
    for (key, value) in tags {
        collected.insert(key.to_owned(), value.to_owned());
    }
    collected
}

/// Returns true when the key marks an element as a point of interest.
///
/// Currently we only treat the `historic` and `tourism` tags as POI markers.
/// Extend this predicate when new tag families must be supported.
fn is_relevant_key(key: &str) -> bool {
    matches!(key, "historic" | "tourism")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn monument_tags() -> Vec<(&'static str, &'static str)> {
        make_monument_tags()
    }

    #[rstest]
    #[case::historic(vec![("historic", "memorial")])]
    #[case::tourism(vec![("tourism", "museum")])]
    #[case::mixed(make_monument_tags())]
    fn detects_relevant_keys(#[case] tags: Vec<(&'static str, &'static str)>) {
        assert!(has_relevant_key(tags.iter().copied()));
    }

    #[rstest]
    #[case::amenity(vec![("amenity", "cafe")])]
    #[case::name(vec![("name", "Pergamon Museum")])]
    fn ignores_irrelevant_keys(#[case] tags: Vec<(&'static str, &'static str)>) {
        assert!(!has_relevant_key(tags.iter().copied()));
    }

    #[rstest]
    fn collects_tags_into_owned_map(monument_tags: Vec<(&'static str, &'static str)>) {
        let collected = collect_tags(monument_tags.iter().copied());

        assert_eq!(collected.get("historic"), Some(&"monument".to_string()));
        assert_eq!(collected.get("name"), Some(&"Victory Column".to_string()));
    }

    fn make_monument_tags() -> Vec<(&'static str, &'static str)> {
        vec![("historic", "monument"), ("name", "Victory Column")]
    }
}
