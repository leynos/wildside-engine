use geo::Coord;
use wildside_core::{PointOfInterest, Tags, build_spatial_index};

fn poi(id: u64, x: f64, y: f64) -> PointOfInterest {
    PointOfInterest::with_empty_tags(id, Coord { x, y })
}

fn tagged_poi(id: u64, x: f64, y: f64, name: &str) -> PointOfInterest {
    PointOfInterest::new(
        id,
        Coord { x, y },
        Tags::from([(String::from("name"), String::from(name))]),
    )
}

#[test]
fn point_of_interest_retains_tags() {
    let poi = tagged_poi(1, 0.0, 0.0, "museum");
    assert_eq!(poi.tags.get("name"), Some(&String::from("museum")));
}

#[test]
fn spatial_index_len_matches_input() {
    let mut pois = vec![poi(1, 0.0, 0.0), poi(2, 1.0, 1.0)];
    let index = build_spatial_index(pois.clone());

    assert_eq!(index.len(), pois.len());
    assert!(!index.is_empty());

    let mut collected: Vec<_> = index.iter().cloned().collect();
    collected.sort_by_key(|poi| poi.id);
    pois.sort_by_key(|poi| poi.id);
    assert_eq!(collected, pois);
}

#[test]
fn query_returns_multiple_pois_inside_bbox() {
    let city_centre = tagged_poi(1, 0.0, 0.0, "city-centre");
    let riverside = tagged_poi(2, 5.0, 1.0, "riverside");
    let plaza = tagged_poi(3, 0.3, 0.2, "plaza");
    let index = build_spatial_index(vec![city_centre.clone(), riverside, plaza.clone()]);

    // Bounding box intersects both central POIs while excluding the riverside
    // landmark, ensuring multi-result queries are supported.
    let mut results = index.query_within(Coord { x: -0.5, y: -0.5 }, Coord { x: 0.6, y: 0.6 });
    results.sort_by_key(|poi| poi.id);

    assert_eq!(results, vec![city_centre, plaza]);
}

#[test]
fn query_outside_bbox_returns_nothing() {
    let index = build_spatial_index(vec![poi(1, 0.0, 0.0), poi(2, 1.0, 1.0)]);
    let results = index.query_within(Coord { x: 10.0, y: 10.0 }, Coord { x: 11.0, y: 11.0 });
    assert!(results.is_empty());
}

#[test]
fn spatial_index_handles_empty_input() {
    let index = build_spatial_index(Vec::new());
    assert_eq!(index.len(), 0);
    assert!(index.is_empty());
    assert_eq!(index.iter().count(), 0);
}
