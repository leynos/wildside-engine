//! Shared helpers used across Wikidata dump operations and sources.

use std::io::{self, BufRead, BufReader, Read};

use futures_util::TryStreamExt;
use tokio_util::io::{StreamReader, SyncIoBridge};

use super::BaseUrl;

/// Trim trailing slashes and fall back to the default Wikidata endpoint.
pub(crate) fn sanitise_base_url(url: impl Into<String>) -> BaseUrl {
    let raw = url.into();
    let trimmed = raw.trim_end_matches('/');
    if trimmed.is_empty() {
        BaseUrl::from("https://dumps.wikimedia.org")
    } else {
        BaseUrl::new(trimmed.to_owned())
    }
}

/// Convert an asynchronous HTTP response into a blocking buffered reader.
pub(crate) fn to_blocking_reader(response: reqwest::Response) -> Box<dyn BufRead + Send> {
    Box::new(BufReader::new(into_blocking_stream(response)))
}

/// Convert an asynchronous HTTP response into a blocking reader.
pub(crate) fn to_sync_reader(response: reqwest::Response) -> Box<dyn Read + Send> {
    Box::new(into_blocking_stream(response))
}

fn into_blocking_stream(response: reqwest::Response) -> impl Read + Send {
    let stream = response.bytes_stream().map_err(io::Error::other);
    SyncIoBridge::new(StreamReader::new(stream))
}
