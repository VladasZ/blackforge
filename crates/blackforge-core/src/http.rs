use std::{io::Read, path::Path};

use flate2::read::GzDecoder;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use tokio::{fs::File, io::AsyncWriteExt, task::spawn_blocking};

use crate::error::{IoContext, Result};

const USER_AGENT: &str = concat!("blackforge/", env!("CARGO_PKG_VERSION"));
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

#[derive(Clone, Debug)]
pub struct Client {
    http: reqwest::Client,
}

impl Client {
    pub fn new() -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder().user_agent(USER_AGENT).build()?,
        })
    }

    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let response = self.http.get(url).send().await?.error_for_status()?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Thunderstore serves the package list as gzip files, not as gzip
    /// transfer encoding, so the body is unpacked here by its magic bytes.
    pub async fn get_json<T: DeserializeOwned + Send + 'static>(&self, url: &str) -> Result<T> {
        let body = self.get_bytes(url).await?;
        spawn_blocking(move || parse_json(&body)).await?
    }

    pub async fn post_bytes(
        &self,
        url: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let request = self
            .http
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body);
        let response = request.send().await?.error_for_status()?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Streams `url` into `dest`. `on_bytes` gets the total received so far.
    pub async fn download(
        &self,
        url: &str,
        dest: &Path,
        mut on_start: impl FnMut(Option<u64>) -> Result<()>,
        mut on_bytes: impl FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        let response = self.http.get(url).send().await?.error_for_status()?;
        on_start(response.content_length())?;
        let mut file = File::create(dest).await.at(dest)?;
        let mut stream = response.bytes_stream();
        let mut received = 0_u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await.at(dest)?;
            received += chunk.len() as u64;
            on_bytes(received)?;
        }
        file.flush().await.at(dest)
    }
}

fn parse_json<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    if body.starts_with(&GZIP_MAGIC) {
        let mut unpacked = Vec::new();
        GzDecoder::new(body)
            .read_to_end(&mut unpacked)
            .at(Path::new("gzip response body"))?;
        return Ok(serde_json::from_slice(&unpacked)?);
    }
    Ok(serde_json::from_slice(body)?)
}
