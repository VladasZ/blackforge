//! The landing page and the few jobs the old nginx config did next to it.
//! Release files live on studio. A browser would block a fetch of the two
//! small json files from another origin, so those are passed through here, and
//! so are the download click reports. Everything else under `/download/` is a
//! redirect, no release bytes flow through this server.

use hilen_server::{
    AppError,
    axum::{
        Router,
        body::Bytes,
        extract::OriginalUri,
        http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
        response::{IntoResponse, Redirect, Response},
        routing::{get, post},
    },
    web_mount,
};
use reqwest::Client;
use rust_embed::RustEmbed;

const STUDIO: &str = "https://gebling-studio.vladas.xyz";

#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
// CI lints and tests the workspace with no page built. The Dockerfile checks
// that the page is there before it builds the server that ships.
#[allow_missing = true]
struct Web;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route(
            "/download/manifest.json",
            get(|| pass_get("/blackforge/download/manifest.json")),
        )
        .route(
            "/download/updater.json",
            get(|| pass_get("/blackforge/download/updater.json")),
        )
        .route("/api/downloads/track", post(pass_track))
        .route("/download/{*rest}", get(to_studio))
        .merge(web_mount::<Web, S>())
}

/// Installed apps have this host baked into their updater address and follow
/// the redirect.
async fn to_studio(OriginalUri(uri): OriginalUri) -> Redirect {
    let path = uri
        .path_and_query()
        .map_or_else(|| uri.path(), |full| full.as_str());
    Redirect::permanent(&format!("{STUDIO}/blackforge{path}"))
}

async fn pass_get(path: &str) -> Result<Response, AppError> {
    let answer = Client::new()
        .get(format!("{STUDIO}{path}"))
        .send()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    relay(answer).await
}

async fn pass_track(headers: HeaderMap, body: Bytes) -> Result<Response, AppError> {
    let mut request = Client::new()
        .post(format!("{STUDIO}/api/downloads/track"))
        .body(body);
    if let Some(kind) = headers.get(CONTENT_TYPE) {
        request = request.header(CONTENT_TYPE, kind);
    }
    let answer = request
        .send()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    relay(answer).await
}

async fn relay(answer: reqwest::Response) -> Result<Response, AppError> {
    let status = StatusCode::from_u16(answer.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let kind = answer.headers().get(CONTENT_TYPE).cloned();
    let body = answer
        .bytes()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;

    let mut response = (status, body).into_response();
    if let Some(kind) = kind {
        response.headers_mut().insert(CONTENT_TYPE, kind);
    }
    Ok(response)
}
