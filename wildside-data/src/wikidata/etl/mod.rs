//! Wikidata entity extraction from dump files.
//!
//! Streams the JSON dump, filters for entities linked from previously ingested
//! OpenStreetMap POIs, and extracts claims that will later populate the local
//! semantic store. The parser is deliberately incremental: it avoids loading the
//! full dump into memory and only yields entities referenced by the OSM ingest
//! report.
#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    io::{BufRead, Read},
};

use serde::Deserialize;
use thiserror::Error;
use wildside_core::PointOfInterest;

const HERITAGE_PROPERTY: &str = "P1435";

/// Mapping between Wikidata entity identifiers and linked POI ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoiEntityLinks {
    links: BTreeMap<String, Vec<u64>>,
}

impl PoiEntityLinks {
    /// Build the mapping from an iterator of [`PointOfInterest`] references.
    ///
    /// # Examples
    /// ```
    /// use geo::Coord;
    /// use wildside_core::{PointOfInterest, Tags};
    /// use wildside_data::wikidata::etl::PoiEntityLinks;
    ///
    /// let poi = PointOfInterest::new(
    ///     1,
    ///     Coord { x: 0.0, y: 0.0 },
    ///     Tags::from([("wikidata".into(), "Q64".into())]),
    /// );
    /// let links = PoiEntityLinks::from_pois([&poi]);
    ///
    /// assert!(links.contains("Q64"));
    /// assert_eq!(links.linked_poi_ids("Q64"), Some(&[1][..]));
    /// ```
    pub fn from_pois<'a, I>(pois: I) -> Self
    where
        I: IntoIterator<Item = &'a PointOfInterest>,
    {
        let mut links: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for poi in pois {
            if let Some(raw) = poi.tags.get("wikidata")
                && let Some(entity_id) = normalise_wikidata_id(raw)
            {
                links.entry(entity_id).or_default().push(poi.id);
            }
        }
        for poi_ids in links.values_mut() {
            poi_ids.sort_unstable();
            poi_ids.dedup();
        }
        Self { links }
    }

    /// Report whether the mapping contains the provided entity identifier.
    #[must_use]
    pub fn contains(&self, entity_id: &str) -> bool {
        self.links.contains_key(entity_id)
    }

    /// Retrieve the POI identifiers linked to the entity.
    #[must_use]
    pub fn linked_poi_ids(&self, entity_id: &str) -> Option<&[u64]> {
        self.links.get(entity_id).map(Vec::as_slice)
    }

    /// Return whether the mapping is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

/// Claims extracted for an entity referenced by one or more POIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityClaims {
    /// The Wikidata entity identifier (e.g., `Q64`).
    pub entity_id: String,
    /// POIs that reference this entity via the `wikidata` tag.
    pub linked_poi_ids: Vec<u64>,
    /// Heritage designation entity identifiers (`P1435` claim targets).
    pub heritage_designations: Vec<String>,
}

impl EntityClaims {
    fn new(
        entity_id: String,
        linked_poi_ids: Vec<u64>,
        heritage_designations: Vec<String>,
    ) -> Self {
        Self {
            entity_id,
            linked_poi_ids,
            heritage_designations,
        }
    }
}

/// Errors that can occur while extracting claims from a Wikidata dump.
#[derive(Debug, Error)]
pub enum WikidataEtlError {
    #[error("failed to read Wikidata dump at line {line}")]
    ReadLine {
        #[source]
        source: std::io::Error,
        line: usize,
    },
    #[error("failed to parse Wikidata entity at line {line}")]
    ParseEntity {
        #[source]
        source: simd_json::Error,
        line: usize,
    },
}

/// Extract claims for entities linked from the supplied POIs.
///
/// The function streams through the dump, ignoring unrelated entities and only
/// returning records that correspond to `wikidata` tags discovered during OSM
/// ingestion. Currently only the heritage designation claim (`P1435`) is
/// captured, but the structure leaves space for additional properties.
///
/// # Examples
/// ```
/// use std::io::Cursor;
/// use geo::Coord;
/// use wildside_core::{PointOfInterest, Tags};
/// use wildside_data::wikidata::etl::{PoiEntityLinks, extract_linked_entity_claims};
///
/// let poi = PointOfInterest::new(
///     1,
///     Coord { x: 13.4, y: 52.5 },
///     Tags::from([("wikidata".into(), "Q64".into())]),
/// );
/// let links = PoiEntityLinks::from_pois([&poi]);
/// let dump = Cursor::new(r#"{"id":"Q64","claims":{"P1435":[{"mainsnak":{"snaktype":"value","datavalue":{"type":"wikibase-entityid","value":{"id":"Q9259"}}}}]}}"#);
/// let claims = extract_linked_entity_claims(dump, &links)?;
///
/// assert_eq!(claims[0].entity_id, "Q64");
/// assert_eq!(claims[0].heritage_designations, vec!["Q9259"]);
/// # Ok::<(), wildside_data::wikidata::etl::WikidataEtlError>(())
/// ```
pub fn extract_linked_entity_claims<R>(
    reader: R,
    links: &PoiEntityLinks,
) -> Result<Vec<EntityClaims>, WikidataEtlError>
where
    R: Read,
{
    if links.is_empty() {
        return Ok(Vec::new());
    }

    let mut buffered = std::io::BufReader::new(reader);
    let mut line = String::new();
    let mut line_number = 0usize;
    let mut extracted = Vec::new();

    loop {
        line.clear();
        match buffered.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                line_number += 1;
            }
            Err(source) => {
                return Err(WikidataEtlError::ReadLine {
                    source,
                    line: line_number + 1,
                });
            }
        }

        let Some(preprocessed) = preprocess_json_line(&line) else {
            continue;
        };

        if let Some(claims) = process_entity_claims(preprocessed, links, line_number)? {
            extracted.push(claims);
        }
    }

    Ok(extracted)
}

fn preprocess_json_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if is_structural_line(trimmed) {
        return None;
    }
    let trimmed = trimmed.trim_start_matches(',').trim();
    let trimmed = if trimmed.ends_with(',') {
        let without_comma = trimmed
            .strip_suffix(',')
            .expect("trimmed.ends_with(',') validated");
        without_comma.trim()
    } else {
        trimmed
    };
    if trimmed.is_empty() || is_structural_line(trimmed) {
        None
    } else {
        Some(trimmed)
    }
}

fn is_structural_line(line: &str) -> bool {
    line.is_empty() || line == "[" || line == "]"
}

fn process_entity_claims(
    json_slice: &str,
    links: &PoiEntityLinks,
    line_number: usize,
) -> Result<Option<EntityClaims>, WikidataEtlError> {
    let mut bytes = json_slice.as_bytes().to_vec();
    let entity: RawEntity = simd_json::from_slice(bytes.as_mut_slice()).map_err(|source| {
        WikidataEtlError::ParseEntity {
            source,
            line: line_number,
        }
    })?;
    let Some(normalised_id) = normalise_wikidata_id(&entity.id) else {
        return Ok(None);
    };
    if !links.contains(&normalised_id) {
        return Ok(None);
    }
    let mut heritage_designations = entity.heritage_designations();
    heritage_designations.sort_unstable();
    heritage_designations.dedup();

    let linked_poi_ids = links
        .linked_poi_ids(&normalised_id)
        .map(|ids| ids.to_vec())
        .unwrap_or_default();

    Ok(Some(EntityClaims::new(
        normalised_id,
        linked_poi_ids,
        heritage_designations,
    )))
}

fn normalise_wikidata_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let last_segment = trimmed.rsplit(['/', '#']).next().unwrap_or(trimmed);
    let final_segment = last_segment
        .rsplit(':')
        .next()
        .unwrap_or(last_segment)
        .trim();
    let mut chars = final_segment.chars();
    let prefix = chars.next()?;
    if !matches!(prefix, 'Q' | 'q') {
        return None;
    }
    let digits: String = chars.collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("Q{digits}"))
}

#[derive(Debug, Deserialize)]
struct RawEntity {
    id: String,
    #[serde(default)]
    claims: BTreeMap<String, Vec<RawClaim>>,
}

impl RawEntity {
    fn heritage_designations(&self) -> Vec<String> {
        self.claims
            .get(HERITAGE_PROPERTY)
            .into_iter()
            .flatten()
            .filter_map(RawClaim::heritage_target)
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct RawClaim {
    #[serde(rename = "mainsnak")]
    main_snak: RawSnak,
}

impl RawClaim {
    fn heritage_target(&self) -> Option<String> {
        self.main_snak.entity_target()
    }
}

#[derive(Debug, Deserialize)]
struct RawSnak {
    #[serde(rename = "snaktype")]
    snak_type: RawSnakType,
    #[serde(rename = "datavalue")]
    data_value: Option<RawDataValue>,
}

impl RawSnak {
    fn entity_target(&self) -> Option<String> {
        if self.snak_type != RawSnakType::Value {
            return None;
        }
        let RawDataValue::Entity { value } = self.data_value.as_ref()? else {
            return None;
        };
        normalise_wikidata_id(&value.id)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawSnakType {
    Value,
    Somevalue,
    Novalue,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawDataValue {
    #[serde(rename = "wikibase-entityid")]
    Entity { value: RawEntityId },
    #[serde(other)]
    Unsupported,
}

#[derive(Debug, Deserialize)]
struct RawEntityId {
    id: String,
}

#[cfg(test)]
mod tests;
