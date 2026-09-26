//! The local door the join plugin knocks on, for the rules of a server at the
//! click on a join button and for a code at Start in character select. The
//! app is signed in with Google, so it asks the backend for them
//! and hands them to the game. It listens on 127.0.0.1 only, and a request
//! must carry the key this app gave the game at its start, see `docs/gate.md`.

use std::{fs, sync::Mutex, thread, time::Instant};

use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use blackforge_api::report::ConnectionReport;
use blackforge_core::{Error, paths::DataDir};
use constant_time_eq::constant_time_eq;
use tiny_http::{Method, Request, Response, Server};
use tokio::runtime::Builder;

use crate::social;

const KEY_HEADER: &str = "X-Blackforge-Key";
const RULES_PATH: &str = "/rules/";
const JOIN_PATH: &str = "/join/";
const REPORT_PATH: &str = "/report";
/// Reports the server did not take wait in the data folder, the newest ones.
const KEEP_REPORTS: usize = 50;
const SIGN_IN: &str = "Sign in to Blackforge to join";
const UNREACHABLE: &str = "Blackforge is not reachable, try again later";
const STARTED_AGAIN: &str = "Blackforge started Valheim again after this game opened, so this game cannot join. Close Valheim and press Play in Blackforge.";
const APP_RESTARTED: &str = "Blackforge was restarted after this game opened, so this game cannot join. Close Valheim and press Play in Blackforge.";

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
    for mut request in server.incoming_requests() {
        let started = Instant::now();
        let url = request.url().to_owned();
        let (status, text) = if url == REPORT_PATH {
            report(&mut request)
        } else {
            runtime.block_on(answer(&request))
        };
        // Every request of the game, a join problem starts here. A 200 of a
        // join door carries the code, so only a refusal logs its text.
        let ms = started.elapsed().as_millis();
        if status < 300 {
            log::info!("the game asked {url}: {status} in {ms} ms");
        } else {
            log::warn!("the game asked {url}: {status} in {ms} ms, {text}");
        }
        let response = Response::from_string(text).with_status_code(status);
        if let Err(error) = request.respond(response) {
            log::warn!("the join bridge did not answer: {error}");
        }
    }
}

/// Why a key is refused, in words the player can act on. None lets the
/// request in.
fn key_refusal(request: &Request) -> Option<&'static str> {
    let Ok(key) = KEY.lock() else {
        return Some(UNREACHABLE);
    };
    // No key means this run of the app never started the game.
    let Some(key) = key.as_deref() else {
        return Some(APP_RESTARTED);
    };
    // The plugin only asks with the key from its start arguments, so a wrong
    // one is from an older start. Steam does not start a second Valheim, the
    // running game keeps the old key.
    let matches = request
        .headers()
        .iter()
        .find(|header| header.field.equiv(KEY_HEADER))
        .is_some_and(|header| constant_time_eq(header.value.as_bytes(), key.as_bytes()));
    (!matches).then_some(STARTED_AGAIN)
}

/// A report of a failed join or a dropped connection, see `docs/reports.md`.
/// It skips the key check, a game with a stale key is one of the failures
/// worth a report, and a report only ever goes to the account of this app.
/// It answers at once and sends in the background, so a slow backend never
/// holds up a join code at the other doors.
fn report(request: &mut Request) -> (u16, String) {
    if request.method() != &Method::Post {
        return (405, "only POST".to_owned());
    }
    let mut body = String::new();
    if let Err(error) = request.as_reader().read_to_string(&mut body) {
        return (400, format!("no report: {error}"));
    }
    let mut report: ConnectionReport = match serde_json::from_str(&body) {
        Ok(report) => report,
        Err(error) => return (400, format!("no report: {error}")),
    };
    env!("CARGO_PKG_VERSION").clone_into(&mut report.app_version);
    report.app_log = app_log();
    report.trim();
    log::info!(
        "connection report: {} {} {} after {:.1}s, {}",
        report.server,
        report.character,
        report.status,
        report.seconds,
        report.message
    );
    let sent = thread::Builder::new()
        .name("connection-report".to_owned())
        .spawn(move || deliver(&report));
    match sent {
        Ok(_) => (202, "taken".to_owned()),
        Err(error) => {
            log::warn!("the connection report did not start: {error}");
            (500, UNREACHABLE.to_owned())
        }
    }
}

/// Sends a report, then the kept ones. A report the server did not take
/// waits in the data folder for the next one that gets through.
fn deliver(report: &ConnectionReport) {
    let result = send(report);
    if let Err(error) = result {
        log::warn!("the connection report did not reach the server, it waits: {error:#}");
        keep(report);
        return;
    }
    log::info!("the connection report reached the server");
    let Ok(dir) = DataDir::locate().map(|data| data.reports_dir()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    for file in files {
        let kept = fs::read_to_string(&file)
            .map_err(anyhow::Error::from)
            .and_then(|text| Ok(serde_json::from_str::<ConnectionReport>(&text)?));
        let kept = match kept {
            Ok(kept) => kept,
            Err(error) => {
                log::warn!("{} did not read, it is dropped: {error:#}", file.display());
                let _ = fs::remove_file(&file);
                continue;
            }
        };
        if let Err(error) = send(&kept) {
            log::warn!("a kept connection report still did not reach the server: {error:#}");
            return;
        }
        match fs::remove_file(&file) {
            Ok(()) => log::info!("sent the kept connection report {}", file.display()),
            Err(error) => log::warn!("{} was sent but not deleted: {error}", file.display()),
        }
    }
}

fn send(report: &ConnectionReport) -> Result<()> {
    if !social::signed_in() {
        return Err(anyhow!("nobody is signed in"));
    }
    let client = social::client()?;
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(client.send_report(report))?;
    Ok(())
}

/// The newest `KEEP_REPORTS` wait, an older one is dropped.
fn keep(report: &ConnectionReport) {
    let result = DataDir::locate()
        .map_err(anyhow::Error::from)
        .and_then(|data| {
            let dir = data.reports_dir();
            fs::create_dir_all(&dir)?;
            let mut kept: Vec<_> = fs::read_dir(&dir)?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            kept.sort();
            for old in kept.iter().rev().skip(KEEP_REPORTS - 1) {
                fs::remove_file(old)?;
            }
            let name = format!("{}.json", chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f"));
            fs::write(dir.join(name), serde_json::to_vec(report)?)?;
            Ok(())
        });
    if let Err(error) = result {
        log::warn!("the connection report is lost, it did not save: {error:#}");
    }
}

/// The log file of this run of the app, the trim keeps its newest part.
fn app_log() -> String {
    let Some(path) = hilen::log_file_path() else {
        return "no app log file".to_owned();
    };
    match fs::read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => format!("{} did not read: {error}", path.display()),
    }
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
    if let Some(refusal) = key_refusal(request) {
        return (403, refusal.to_owned());
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
