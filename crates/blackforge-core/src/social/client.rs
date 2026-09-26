//! The calls to the blackforge server. The session token comes from the
//! Google login of the frontend, this crate never sees how it was made.

use std::time::Duration;

use blackforge_api::{
    ApiError, FoundUser, FriendName, Friends, Me, Search, SetUsername, SharedProfile, Status,
    gate::{AddMember, JoinCode, JoinRules, Member},
    report::ConnectionReport,
    servers::{SaveServer, Server},
    setup::{Account, HistoryRow, Restore, Save, Saved},
};
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::from_slice;

use crate::error::{Error, Result};

pub const SERVER: &str = "https://blackforge.vladas.xyz";

const USER_AGENT: &str = concat!("blackforge/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug)]
pub struct SocialClient {
    server: String,
    token: String,
    http: reqwest::Client,
}

impl SocialClient {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        Self::with_server(SERVER, token)
    }

    pub fn with_server(server: &str, token: impl Into<String>) -> Result<Self> {
        Ok(Self {
            server: server.trim_end_matches('/').to_owned(),
            token: token.into(),
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .build()?,
        })
    }

    pub async fn me(&self) -> Result<Me> {
        self.read(self.request(Method::GET, "/api/me")).await
    }

    /// The name cannot be changed later, so the frontend asks twice.
    pub async fn set_username(&self, username: &str) -> Result<Me> {
        let body = SetUsername {
            username: username.to_owned(),
        };
        self.read(self.request(Method::POST, "/api/me/username").json(&body))
            .await
    }

    pub async fn friends(&self) -> Result<Friends> {
        self.read(self.request(Method::GET, "/api/friends")).await
    }

    pub async fn search_users(&self, start: &str) -> Result<Vec<FoundUser>> {
        let query = Search {
            q: start.to_owned(),
        };
        self.read(self.request(Method::GET, "/api/users/search").query(&query))
            .await
    }

    pub async fn request_friend(&self, username: &str) -> Result<()> {
        self.friend_action("request", username).await
    }

    pub async fn accept_friend(&self, username: &str) -> Result<()> {
        self.friend_action("accept", username).await
    }

    pub async fn decline_friend(&self, username: &str) -> Result<()> {
        self.friend_action("decline", username).await
    }

    /// Ends a friendship for both sides, or takes back a request of my own.
    pub async fn remove_friend(&self, username: &str) -> Result<()> {
        self.friend_action("remove", username).await
    }

    pub async fn upload_profile(&self, profile: &SharedProfile) -> Result<()> {
        self.send(self.request(Method::PUT, "/api/profile").json(profile))
            .await
    }

    pub async fn friend_profile(&self, username: &str) -> Result<SharedProfile> {
        let path = format!("/api/friends/{username}/profile");
        self.read(self.request(Method::GET, &path)).await
    }

    pub async fn set_status(&self, in_game: bool) -> Result<()> {
        self.send(
            self.request(Method::POST, "/api/status")
                .json(&Status { in_game }),
        )
        .await
    }

    /// The account and the setup every machine of it installs.
    pub async fn sync_head(&self) -> Result<Account> {
        self.read(self.request(Method::GET, "/api/sync")).await
    }

    pub async fn sync_save(&self, save: &Save) -> Result<Saved> {
        self.read(self.request(Method::POST, "/api/sync").json(save))
            .await
    }

    pub async fn sync_history(&self) -> Result<Vec<HistoryRow>> {
        self.read(self.request(Method::GET, "/api/sync/history"))
            .await
    }

    pub async fn sync_restore(&self, restore: &Restore) -> Result<Saved> {
        self.read(
            self.request(Method::POST, "/api/sync/restore")
                .json(restore),
        )
        .await
    }

    pub async fn create_server(&self, save: &SaveServer) -> Result<Server> {
        self.read(self.request(Method::POST, "/api/servers").json(save))
            .await
    }

    pub async fn update_server(&self, id: &str, save: &SaveServer) -> Result<Server> {
        let path = format!("/api/servers/{id}");
        self.read(self.request(Method::PUT, &path).json(save)).await
    }

    pub async fn delete_server(&self, id: &str) -> Result<()> {
        let path = format!("/api/servers/{id}");
        self.send(self.request(Method::DELETE, &path)).await
    }

    /// Who may join my servers, one list for all of them.
    pub async fn members(&self) -> Result<Vec<Member>> {
        self.read(self.request(Method::GET, "/api/members")).await
    }

    pub async fn add_member(&self, username: &str) -> Result<Vec<Member>> {
        let body = AddMember {
            username: username.to_owned(),
        };
        self.read(self.request(Method::POST, "/api/members").json(&body))
            .await
    }

    pub async fn remove_member(&self, username: &str) -> Result<Vec<Member>> {
        let path = format!("/api/members/{username}");
        self.read(self.request(Method::DELETE, &path)).await
    }

    /// A one time code the game trades for a join of the server.
    /// The member check and the rules of a server, asked at the click on a
    /// join button. It makes no code.
    pub async fn join_rules(&self, server_id: &str) -> Result<JoinRules> {
        let path = format!("/api/servers/{server_id}/rules");
        self.read(self.request(Method::POST, &path)).await
    }

    /// A one time join code, asked at Start in character select.
    pub async fn join_code(&self, server_id: &str) -> Result<JoinCode> {
        let path = format!("/api/servers/{server_id}/join");
        self.read(self.request(Method::POST, &path)).await
    }

    /// What the game saw at a failed join or a dropped connection.
    pub async fn send_report(&self, report: &ConnectionReport) -> Result<()> {
        self.send(self.request(Method::POST, "/api/reports").json(report))
            .await
    }

    async fn friend_action(&self, action: &str, username: &str) -> Result<()> {
        let body = FriendName {
            username: username.to_owned(),
        };
        let path = format!("/api/friends/{action}");
        self.send(self.request(Method::POST, &path).json(&body))
            .await
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.server))
            .bearer_auth(&self.token)
    }

    async fn read<Out: DeserializeOwned>(&self, request: RequestBuilder) -> Result<Out> {
        Ok(from_slice(&answer(request).await?)?)
    }

    async fn send(&self, request: RequestBuilder) -> Result<()> {
        answer(request).await.map(drop)
    }
}

/// The body of a good answer. A refusal comes back as the words the server
/// wrote for the user, a dead session as its own error so the frontend can
/// send the user to sign in again.
async fn answer(request: RequestBuilder) -> Result<Vec<u8>> {
    let response = request.send().await?;
    let status = response.status();
    let body = response.bytes().await?.to_vec();

    if status.is_success() {
        return Ok(body);
    }
    Err(refusal(status, &body))
}

fn refusal(status: StatusCode, body: &[u8]) -> Error {
    if status == StatusCode::UNAUTHORIZED {
        return Error::NotSignedIn;
    }
    match from_slice::<ApiError>(body) {
        Ok(refused) => Error::Server(refused.error),
        Err(_) => Error::Server(format!("the server answered {status}")),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::refusal;
    use crate::error::Error;

    #[test]
    fn a_dead_session_is_its_own_error() {
        assert!(matches!(
            refusal(StatusCode::UNAUTHORIZED, br#"{"error":"unauthorized"}"#),
            Error::NotSignedIn
        ));
    }

    #[test]
    fn a_refusal_carries_the_words_of_the_server() {
        let error = refusal(
            StatusCode::BAD_REQUEST,
            br#"{"error":"the username anna is taken"}"#,
        );
        assert_eq!(error.to_string(), "the username anna is taken");
    }

    #[test]
    fn a_broken_answer_still_says_something() {
        let error = refusal(StatusCode::BAD_GATEWAY, b"<html>bad gateway</html>");
        assert_eq!(error.to_string(), "the server answered 502 Bad Gateway");
    }
}
