//! Unit tests for the Wikidata ETL parser.

mod behaviour;

use super::{
    EntityClaims, PoiEntityLinks, WikidataEtlError, extract_linked_entity_claims,
    normalise_wikidata_id,
};
use geo::Coord;
use rstest::{fixture, rstest};
use std::io::Cursor;
use wildside_core::{PointOfInterest, Tags};

#[fixture]
fn poi_with_wikidata() -> PointOfInterest {
    PointOfInterest::new(
        7,
        Coord {
            x: 13.404954,
            y: 52.520008,
        },
        Tags::from([("wikidata".into(), "Q64".into())]),
    )
}

#[rstest]
fn builds_links_from_pois(poi_with_wikidata: PointOfInterest) {
    let links = PoiEntityLinks::from_pois([&poi_with_wikidata, &poi_with_wikidata]);

    assert!(links.contains("Q64"));
    assert_eq!(links.linked_poi_ids("Q64"), Some(&[7][..]));
}

#[rstest]
fn ignores_invalid_wikidata_tags() {
    let poi = PointOfInterest::new(
        9,
        Coord { x: 0.0, y: 0.0 },
        Tags::from([("wikidata".into(), "not-an-id".into())]),
    );

    let links = PoiEntityLinks::from_pois([&poi]);

    assert!(links.is_empty());
}

#[rstest]
fn normalises_http_urls() {
    assert_eq!(
        normalise_wikidata_id("https://www.wikidata.org/wiki/Q9259"),
        Some("Q9259".to_string())
    );
}

#[rstest]
fn extract_claims_for_linked_entity(poi_with_wikidata: PointOfInterest) {
    let links = PoiEntityLinks::from_pois([&poi_with_wikidata]);
    let dump = Cursor::new(
        r#"{"id":"Q64","claims":{"P1435":[{"mainsnak":{"snaktype":"value","datavalue":{"type":"wikibase-entityid","value":{"id":"Q9259"}}}}]}}"#,
    );

    let claims = extract_linked_entity_claims(dump, &links).expect("parsing should succeed");

    assert_eq!(
        claims,
        vec![EntityClaims::new(
            "Q64".into(),
            vec![7],
            vec!["Q9259".into()]
        )]
    );
}

#[rstest]
fn skips_entities_without_links(poi_with_wikidata: PointOfInterest) {
    let links = PoiEntityLinks::from_pois([&poi_with_wikidata]);
    let dump = Cursor::new(
        r#"{"id":"Q123","claims":{"P1435":[{"mainsnak":{"snaktype":"value","datavalue":{"type":"wikibase-entityid","value":{"id":"Q9259"}}}}]}}"#,
    );

    let claims = extract_linked_entity_claims(dump, &links).expect("parsing should succeed");

    assert!(claims.is_empty());
}

#[rstest]
fn ignores_non_value_snaks(poi_with_wikidata: PointOfInterest) {
    let links = PoiEntityLinks::from_pois([&poi_with_wikidata]);
    let dump =
        Cursor::new(r#"{"id":"Q64","claims":{"P1435":[{"mainsnak":{"snaktype":"novalue"}}]}}"#);

    let claims = extract_linked_entity_claims(dump, &links).expect("parsing should succeed");

    assert_eq!(
        claims,
        vec![EntityClaims::new("Q64".into(), vec![7], Vec::new())]
    );
}

#[rstest]
fn reports_parse_errors(poi_with_wikidata: PointOfInterest) {
    let links = PoiEntityLinks::from_pois([&poi_with_wikidata]);
    let dump = Cursor::new(r#"{"id":"Q64","claims": ["#);

    let err = extract_linked_entity_claims(dump, &links).expect_err("parsing should fail");

    let WikidataEtlError::ParseEntity { line, .. } = err else {
        panic!("expected a parse error");
    };
    assert_eq!(line, 1, "malformed JSON should be flagged on line 1");
}
