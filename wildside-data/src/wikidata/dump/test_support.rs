//! Shared fixtures for Wikidata dump tests.
use std::io::{BufRead, Cursor, Write};

use async_trait::async_trait;

use super::{BaseUrl, DumpSource, TransportError};

/// Stub [`DumpSource`] implementation backed by in-memory data.
#[derive(Debug, Clone)]
pub struct StubSource {
    base_url: BaseUrl,
    manifest: Vec<u8>,
    archive: Vec<u8>,
}

impl StubSource {
    /// Construct a stub source with explicit base URL, manifest, and archive.
    pub fn new(base_url: BaseUrl, manifest: Vec<u8>, archive: Vec<u8>) -> Self {
        Self {
            base_url,
            manifest,
            archive,
        }
    }

    /// Construct a stub source using the provided manifest and archive bytes.
    ///
    /// The base URL defaults to `https://example.org` to keep scenarios concise.
    pub fn with_manifest(manifest: Vec<u8>, archive: Vec<u8>) -> Self {
        Self::new(BaseUrl::from("https://example.org"), manifest, archive)
    }

    /// Access the in-memory archive bytes.
    pub fn archive(&self) -> &[u8] {
        &self.archive
    }
}

#[async_trait(?Send)]
impl DumpSource for StubSource {
    fn base_url(&self) -> &BaseUrl {
        &self.base_url
    }

    async fn fetch_status(&self) -> Result<Box<dyn BufRead + Send>, TransportError> {
        Ok(Box::new(Cursor::new(self.manifest.clone())))
    }

    async fn download_archive(
        &self,
        url: &str,
        sink: &mut dyn Write,
    ) -> Result<u64, TransportError> {
        sink.write_all(&self.archive)
            .map_err(|source| TransportError::Network {
                url: url.to_owned(),
                source,
            })?;
        let length = u64::try_from(self.archive.len()).expect("archive length should fit in u64");
        Ok(length)
    }
}
