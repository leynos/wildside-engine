//! Facilities for discovering and downloading Wikidata dump artefacts.
#![forbid(unsafe_code)]

mod error;
mod log;
mod ops;
mod source;
mod types;
mod util;

#[cfg(any(test, doc))]
mod test_support;
#[cfg(any(test, doc))]
pub use test_support::{StubSource, block_on_for_tests};

pub use error::{TransportError, WikidataDumpError};
pub use log::DownloadLog;
pub use ops::{download_descriptor, download_latest_dump, resolve_latest_descriptor};
pub use source::{DEFAULT_USER_AGENT, DumpSource, HttpDumpSource};
pub use types::{BaseUrl, DownloadOptions, DownloadReport, DumpDescriptor, DumpFileName, DumpUrl};

#[cfg(test)]
mod tests;
