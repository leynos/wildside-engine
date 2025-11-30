//! Resolve sitelink counts for POIs from `SQLite` or embedded tags.
#![forbid(unsafe_code)]

use rusqlite::{CachedStatement, Connection, OptionalExtension};

use crate::{PopularityError, SITELINK_TABLE};

pub(crate) enum SitelinkResolver<'conn> {
    Db { statement: CachedStatement<'conn> },
    TagsOnly,
}

impl<'conn> SitelinkResolver<'conn> {
    pub(crate) fn new(connection: &'conn Connection) -> Result<Self, PopularityError> {
        let has_table: bool = connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
                [SITELINK_TABLE],
                |_| Ok(true),
            )
            .optional()
            .map_err(|source| PopularityError::Query {
                operation: "probe sitelink table",
                source,
            })?
            .unwrap_or(false);

        if has_table {
            let query =
                format!("SELECT sitelink_count FROM {SITELINK_TABLE} WHERE entity_id = ?1 LIMIT 1");
            let statement = connection
                .prepare_cached(query.as_str())
                .map_err(|source| PopularityError::Query {
                    operation: "prepare sitelink lookup",
                    source,
                })?;
            Ok(Self::Db { statement })
        } else {
            Ok(Self::TagsOnly)
        }
    }

    pub(crate) fn sitelink_count(
        &mut self,
        entity_id: Option<&str>,
        tags: &str,
        poi_id: u64,
    ) -> Result<u32, PopularityError> {
        let db_value = match (self, entity_id) {
            (Self::Db { statement }, Some(id)) => statement
                .query_row([id], |row| row.get(0))
                .optional()
                .map_err(|source| PopularityError::Query {
                    operation: "lookup sitelink count",
                    source,
                })?,
            _ => None,
        };

        if let Some(raw) = db_value {
            return i64_to_u32(raw, poi_id);
        }

        if let Some(raw) = parse_sitelinks_from_tags(tags, poi_id)? {
            return i64_to_u32(raw, poi_id);
        }

        Ok(0)
    }
}

fn i64_to_u32(value: i64, poi_id: u64) -> Result<u32, PopularityError> {
    u32::try_from(value).map_err(|_| PopularityError::InvalidSitelinkCount { poi_id, raw: value })
}

pub(crate) fn parse_sitelinks_from_tags(
    tags: &str,
    poi_id: u64,
) -> Result<Option<i64>, PopularityError> {
    let parsed: serde_json::Value = serde_json::from_str(tags)
        .map_err(|source| PopularityError::ParseTags { poi_id, source })?;
    let Some(object) = parsed.as_object() else {
        return Ok(None);
    };
    let candidate = object
        .get("sitelinks")
        .or_else(|| object.get("sitelink_count"));
    let Some(value) = candidate else {
        return Ok(None);
    };
    interpret_sitelink_value(value, poi_id)
}

fn extract_number_value(
    value: &serde_json::Value,
    poi_id: u64,
) -> Result<Option<i64>, PopularityError> {
    value.as_i64().map_or_else(
        || {
            Err(PopularityError::InvalidSitelinkCountJson {
                poi_id,
                raw_json: value.to_string(),
            })
        },
        |v| Ok(Some(v)),
    )
}

fn extract_string_value(
    value: &serde_json::Value,
    poi_id: u64,
) -> Result<Option<i64>, PopularityError> {
    let raw = value.as_str().unwrap_or_default().trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let parsed_value =
        raw.parse::<i64>()
            .map_err(|_| PopularityError::InvalidSitelinkCountJson {
                poi_id,
                raw_json: raw.to_owned(),
            })?;
    Ok(Some(parsed_value))
}

fn interpret_sitelink_value(
    value: &serde_json::Value,
    poi_id: u64,
) -> Result<Option<i64>, PopularityError> {
    if value.is_null() {
        return Ok(None);
    }
    if value.is_number() {
        return extract_number_value(value, poi_id);
    }
    if value.is_string() {
        return extract_string_value(value, poi_id);
    }

    Err(PopularityError::InvalidSitelinkCountJson {
        poi_id,
        raw_json: value.to_string(),
    })
}
