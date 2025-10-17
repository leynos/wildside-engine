//! Behavioural coverage for extracting linked Wikidata claims.

use super::super::{extract_linked_entity_claims, EntityClaims, PoiEntityLinks, WikidataEtlError};
use geo::Coord;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::{cell::RefCell, io::Cursor};
use wildside_core::{PointOfInterest, Tags};

#[fixture]
fn poi_links() -> RefCell<Option<PoiEntityLinks>> {
    RefCell::new(None)
}

#[fixture]
fn dump_bytes() -> RefCell<Vec<u8>> {
    RefCell::new(Vec::new())
}

#[fixture]
fn extraction_result() -> RefCell<Option<Result<Vec<EntityClaims>, WikidataEtlError>>> {
    RefCell::new(None)
}

fn berlin_poi() -> PointOfInterest {
    PointOfInterest::new(
        11,
        Coord { x: 13.404954, y: 52.520008 },
        Tags::from([("wikidata".into(), "Q64".into())]),
    )
}

#[given("an OSM ingest report containing linked POIs")]
fn linked_pois(#[from(poi_links)] cell: &RefCell<Option<PoiEntityLinks>>) {
    *cell.borrow_mut() = Some(PoiEntityLinks::from_pois([&berlin_poi()]));
}

#[given("a dump containing a heritage claim for the linked entity")]
fn dump_with_heritage(#[from(dump_bytes)] cell: &RefCell<Vec<u8>>) {
    *cell.borrow_mut() = br#"{"id":"Q64","claims":{"P1435":[{"mainsnak":{"snaktype":"value","datavalue":{"type":"wikibase-entityid","value":{"id":"Q9259"}}}}]}}"#.to_vec();
}

#[given("a dump with malformed JSON for the linked entity")]
fn dump_with_error(#[from(dump_bytes)] cell: &RefCell<Vec<u8>>) {
    *cell.borrow_mut() = br#"{"id":"Q64","claims": ["#.to_vec();
}

#[when("I extract the linked claims")]
fn extract_claims(
    #[from(poi_links)] links_cell: &RefCell<Option<PoiEntityLinks>>,
    #[from(dump_bytes)] bytes_cell: &RefCell<Vec<u8>>,
    #[from(extraction_result)] result_cell: &RefCell<Option<Result<Vec<EntityClaims>, WikidataEtlError>>>,
) {
    let links = links_cell
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("POI links must be initialised"));
    let bytes = bytes_cell.borrow().clone();
    let cursor = Cursor::new(bytes);
    let outcome = extract_linked_entity_claims(cursor, &links);
    *result_cell.borrow_mut() = Some(outcome);
}

#[then("the UNESCO heritage designation is recorded")]
fn heritage_recorded(
    #[from(extraction_result)] result_cell: &RefCell<Option<Result<Vec<EntityClaims>, WikidataEtlError>>>,
) {
    let outcome = result_cell
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("extraction result must be present"));
    let claims = match outcome {
        Ok(claims) => claims,
        Err(err) => panic!("expected success: {err}"),
    };
    assert_eq!(
        claims,
        vec![EntityClaims::new("Q64".into(), vec![11], vec!["Q9259".into()])]
    );
}

#[then("a parse error is reported")]
fn parse_error(
    #[from(extraction_result)] result_cell: &RefCell<Option<Result<Vec<EntityClaims>, WikidataEtlError>>>,
) {
    let outcome = result_cell
        .borrow()
        .as_ref()
        .cloned()
        .unwrap_or_else(|| panic!("extraction result must be present"));
    match outcome {
        Ok(_) => panic!("expected an error for malformed JSON"),
        Err(WikidataEtlError::ParseEntity { .. }) => {}
        Err(other) => panic!("unexpected error type: {other}"),
    }
}

#[scenario(path = "tests/features/extract_wikidata_claims.feature", index = 0)]
fn extract_heritage_claims() {}

#[scenario(path = "tests/features/extract_wikidata_claims.feature", index = 1)]
fn report_parse_failure() {}
