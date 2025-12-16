//! OSRM API response types for the Table service.
//!
//! This module provides deserialisation types for the OSRM Table API response
//! format. The Table API computes the duration of the fastest route between all
//! pairs of supplied coordinates.
//!
//! See: <http://project-osrm.org/docs/v5.24.0/api/#table-service>

use serde::Deserialize;

/// OSRM Table API response.
///
/// The response contains either a duration matrix on success or an error
/// message on failure. The `code` field indicates the response status.
#[derive(Debug, Deserialize)]
pub struct TableResponse {
    /// Status code from OSRM.
    ///
    /// Common values:
    /// - `"Ok"` - Request was successful
    /// - `"InvalidQuery"` - Invalid query parameters
    /// - `"InvalidOptions"` - Invalid option combination
    /// - `"NoTable"` - Table computation failed
    pub code: String,

    /// Optional error message when `code` is not `"Ok"`.
    pub message: Option<String>,

    /// Matrix of durations in seconds.
    ///
    /// `durations[i][j]` is the travel time from the i-th to the j-th
    /// coordinate. Values are `None` when no route exists between a pair.
    pub durations: Option<Vec<Vec<Option<f64>>>>,
}

impl TableResponse {
    /// Check if the response indicates success.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.code == "Ok"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialise_success_response() {
        let json = r#"{
            "code": "Ok",
            "durations": [[0.0, 120.5], [120.5, 0.0]]
        }"#;

        let response: TableResponse = serde_json::from_str(json).expect("should deserialise");

        assert!(response.is_ok());
        assert!(response.message.is_none());
        let durations = response.durations.expect("should have durations");
        assert_eq!(durations.len(), 2);
        assert_eq!(durations[0][0], Some(0.0));
        assert_eq!(durations[0][1], Some(120.5));
    }

    #[test]
    fn deserialise_error_response() {
        let json = r#"{
            "code": "InvalidQuery",
            "message": "Coordinates are invalid"
        }"#;

        let response: TableResponse = serde_json::from_str(json).expect("should deserialise");

        assert!(!response.is_ok());
        assert_eq!(
            response.message,
            Some("Coordinates are invalid".to_string())
        );
        assert!(response.durations.is_none());
    }

    #[test]
    fn deserialise_response_with_nulls() {
        let json = r#"{
            "code": "Ok",
            "durations": [[0.0, null], [null, 0.0]]
        }"#;

        let response: TableResponse = serde_json::from_str(json).expect("should deserialise");

        assert!(response.is_ok());
        let durations = response.durations.expect("should have durations");
        assert_eq!(durations[0][1], None);
        assert_eq!(durations[1][0], None);
    }
}
