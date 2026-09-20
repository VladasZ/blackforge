//! The two calls the page makes: read the release manifest, and report a
//! download click so studio's stats keep counting them.

use anyhow::Result;
use hilen::net;

use crate::{
    model::Manifest,
    net::{manifest_url, track_url},
};

pub async fn manifest() -> Result<Manifest> {
    net::get(manifest_url()).await
}

/// A failed click report must never prevent a download.
pub async fn track(os: &str, version: &str, arch: Option<&str>) {
    #[derive(serde::Serialize)]
    struct Track<'a> {
        app: &'static str,
        os: &'a str,
        arch: Option<&'a str>,
        version: &'a str,
    }
    let body = Track {
        app: "blackforge",
        os,
        arch,
        version,
    };

    let result: Result<()> = net::post(track_url(), body).await;

    if let Err(error) = result {
        log::warn!("Failed to report a download click: {error}");
    }
}
