//! OSM element ID encoding utilities.
//!
//! Encodes signed OSM element identifiers into a single `u64` namespace by
//! reserving the top two bits for the element kind:
//! - `00` = node
//! - `01` = way
//! - `10` = relation
//!
//! The remaining 62 bits store the raw non-negative identifier. Out-of-range
//! or negative inputs yield `None` and emit a warning.
use log::warn;

/// Top two bits encode element type: 00=node, 01=way, 10=relation. Remaining 62 bits carry the raw ID.
const WAY_ID_PREFIX: u64 = 1 << 62;
const REL_ID_PREFIX: u64 = 1 << 63;
const TYPE_ID_MASK: u64 = (1 << 62) - 1;

#[derive(Copy, Clone, Debug)]
pub(super) enum OsmElementKind {
    Node,
    Way,
    Relation,
}

/// Encode an OSM element ID into the unified `u64` POI ID space.
///
/// Returns `None` and logs a warning when `raw_id` is negative or exceeds the
/// supported 62-bit range.
#[must_use]
pub(super) fn encode_element_id(kind: OsmElementKind, raw_id: i64) -> Option<u64> {
    match u64::try_from(raw_id) {
        Ok(base) => {
            if base > TYPE_ID_MASK {
                warn!(
                    "Skipped OSM element: kind={:?}, raw_id={} (exceeds supported maximum {})",
                    kind, raw_id, TYPE_ID_MASK
                );
                return None;
            }
            let prefix = match kind {
                OsmElementKind::Node => 0,
                OsmElementKind::Way => WAY_ID_PREFIX,
                OsmElementKind::Relation => REL_ID_PREFIX,
            };
            Some(prefix | base)
        }
        Err(_) => {
            warn!(
                "Skipped OSM element: kind={:?}, raw_id={} (negative identifiers are unsupported)",
                kind, raw_id
            );
            None
        }
    }
}
