//! The local door the join plugin knocks on, for the rules of a server at the
//! click on a join button and for a code at Start in character select. The
//! app is signed in with Google, so it asks the backend for them
//! and hands them to the game. It listens on 127.0.0.1 only, and a request
//! must carry the key this app gave the game at its start, see `docs/gate.md`.

use std::{sync::Mutex, thread};

use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use blackforge_core::Error;
use constant_time_eq::constant_time_eq;
use tiny_http::{Method, Request, Response, Server};
use tokio::runtime::Builder;

use crate::social;

const KEY_HEADER: &str = "X-Blackforge-Key";
const RULES_PATH: &str = "/rules/";
const JOIN_PATH: &str = "/join/";
const SIGN_IN: &str = "Sign in to Blackforge to join";
const UNREACHABLE: &str = "Blackforge is not reachable, try again later";

/// The port of the door, it opens at the first start of the game.
static PORT: Mutex<Option<u16>> = Mutex::new(None);
/// The key of the last game start. None before the first one.
static KEY: Mutex<Option<String>> = Mutex::new(None);

/// Opens the door once per run of the app and gives its port.
pub fn open() -> Result<u16> {
    let mut port = PORT.lock().map_err(|_| anyhow!("the bridge lock broke"))?;
    if let Some(port) = *port {
        return Ok(port);
    }
    let server = Server::http("127.0.0.1:0").map_err(|error| anyhow!("{error}"))?;
    let opened = server
        .server_addr()
        .to_ip()
        .context("the join bridge has no ip address")?
        .port();
    thread::Builder::new()
        .name("join-bridge".to_owned())
        .spawn(move || serve(&server))
        .context("the join bridge did not start")?;
    log::info!("the join bridge listens on 127.0.0.1:{opened}");
    *port = Some(opened);
    Ok(opened)
}

/// A new key for a new start of the game. The key of the start before stops
/// working.
pub fn new_key() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| anyhow!("no random key: {error}"))?;
    let key = URL_SAFE_NO_PAD.encode(bytes);
    *KEY.lock().map_err(|_| anyhow!("the bridge lock broke"))? = Some(key.clone());
    Ok(key)
}

fn serve(server: &Server) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            log::error!("the join bridge has no runtime: {error}");
            return;
        }
    };
    for request in server.incoming_requests() {
        let (status, text) = runtime.block_on(answer(&request));
        let response = Response::from_string(text).with_status_code(status);
        if let Err(error) = request.respond(response) {
            log::warn!("the join bridge did not answer: {error}");
        }
    }
}

fn key_matches(request: &Request) -> bool {
    let Ok(key) = KEY.lock() else {
        return false;
    };
    let Some(key) = key.as_deref() else {
        return false;
    };
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(KEY_HEADER))
        .is_some_and(|header| constant_time_eq(header.value.as_bytes(), key.as_bytes()))
}

/// What the plugin asks for.
enum Door {
    /// At the click on a join button, the member check and the rules.
    Rules,
    /// At Start in character select, the one time code.
    Join,
}

/// The status and the text the plugin shows. A 200 carries the `JoinRules` or
/// the `JoinCode` of the backend as JSON.
async fn answer(request: &Request) -> (u16, String) {
    if request.method() != &Method::Post {
        return (405, "only POST".to_owned());
    }
    let url = request.url();
    let (door, server_id) = if let Some(id) = url.strip_prefix(RULES_PATH) {
        (Door::Rules, id)
    } else if let Some(id) = url.strip_prefix(JOIN_PATH) {
        (Door::Join, id)
    } else {
        return (404, "no such door".to_owned());
    };
    if !key_matches(request) {
        return (403, "This game was not started by Blackforge".to_owned());
    }
    if !social::signed_in() {
        return (401, SIGN_IN.to_owned());
    }
    let client = match social::client() {
        Ok(client) => client,
        Err(error) => {
            log::warn!("no client for the join plugin: {error:#}");
            return (500, UNREACHABLE.to_owned());
        }
    };
    let answer = match door {
        Door::Rules => client
            .join_rules(server_id)
            .await
            .and_then(|rules| Ok(serde_json::to_string(&rules)?)),
        Door::Join => client
            .join_code(server_id)
            .await
            .and_then(|code| Ok(serde_json::to_string(&code)?)),
    };
    match answer {
        Ok(text) => (200, text),
        Err(Error::NotSignedIn) => (401, SIGN_IN.to_owned()),
        Err(Error::Server(words)) => (403, words),
        Err(error) => {
            log::warn!("no answer for the join plugin: {error:#}");
            (502, UNREACHABLE.to_owned())
        }
    }
}
