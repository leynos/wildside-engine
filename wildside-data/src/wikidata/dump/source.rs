use async_trait::async_trait;
use reqwest::header::USER_AGENT;
use reqwest::{Client, Response};
use std::io::{self, BufRead, Write};

use super::util::{sanitise_base_url, to_blocking_reader, to_sync_reader};
use super::{BaseUrl, DumpUrl, TransportError};

pub const DEFAULT_USER_AGENT: &str = "wildside-wikidata-etl/0.1";
const STATUS_PATH: &str = "/wikidatawiki/entities/dumpstatus.json";

#[async_trait(?Send)]
pub trait DumpSource {
    /// Base URL of the dump endpoint.
    fn base_url(&self) -> &BaseUrl;
    /// Fetch the dump status manifest.
    async fn fetch_status(&self) -> Result<Box<dyn BufRead + Send>, TransportError>;
    /// Stream the archive identified by `url` into `sink`.
    async fn download_archive(
        &self,
        url: &str,
        sink: &mut dyn Write,
    ) -> Result<u64, TransportError>;
}

/// HTTP implementation of [`DumpSource`].
#[derive(Debug)]
pub struct HttpDumpSource {
    client: Client,
    base_url: BaseUrl,
    user_agent: String,
}

impl HttpDumpSource {
    /// Construct an HTTP-backed dump source.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("client builder only fails with invalid configuration");
        Self {
            client,
            base_url: sanitise_base_url(base_url),
            user_agent: DEFAULT_USER_AGENT.to_owned(),
        }
    }

    /// Override the default user agent string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    fn status_url(&self) -> DumpUrl {
        DumpUrl::new(format!("{}{}", self.base_url.as_ref(), STATUS_PATH))
    }

    async fn call(&self, url: &str) -> Result<Response, TransportError> {
        self.client
            .get(url)
            .header(USER_AGENT, self.user_agent.as_str())
            .send()
            .await
            .map_err(|err| convert_reqwest_error(err, url))?
            .error_for_status()
            .map_err(|err| convert_reqwest_error(err, url))
    }
}

#[async_trait(?Send)]
impl DumpSource for HttpDumpSource {
    fn base_url(&self) -> &BaseUrl {
        &self.base_url
    }

    async fn fetch_status(&self) -> Result<Box<dyn BufRead + Send>, TransportError> {
        use std::time::Duration;

        let url = self.status_url();
        let response = self
            .client
            .get(url.as_ref())
            .timeout(Duration::from_secs(15))
            .header(USER_AGENT, self.user_agent.as_str())
            .send()
            .await
            .map_err(|err| convert_reqwest_error(err, url.as_ref()))?
            .error_for_status()
            .map_err(|err| convert_reqwest_error(err, url.as_ref()))?;
        Ok(to_blocking_reader(response))
    }

    async fn download_archive(
        &self,
        url: &str,
        sink: &mut dyn Write,
    ) -> Result<u64, TransportError> {
        let response = self.call(url).await?;
        let mut reader = to_sync_reader(response);
        io::copy(&mut reader, sink).map_err(|source| TransportError::Network {
            url: url.to_owned(),
            source,
        })
    }
}

fn convert_reqwest_error(error: reqwest::Error, url: &str) -> TransportError {
    if let Some(status) = error.status() {
        return TransportError::Http {
            url: url.to_owned(),
            status: status.as_u16(),
            message: error.to_string(),
        };
    }

    let kind = if error.is_timeout() {
        io::ErrorKind::TimedOut
    } else {
        io::ErrorKind::Other
    };
    TransportError::Network {
        url: url.to_owned(),
        source: io::Error::new(kind, error),
    }
}
