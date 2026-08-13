use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::model::{
    BrowseResponse, CleanResponse, DaemonInfo, HealthResponse, PrepareResponse, Registry,
    SessionRecord, SessionRequest,
};
use crate::storage;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct Daemon;

struct AppState {
    registry: Registry,
    browser_opened: bool,
    events: Arc<EventHub>,
    watcher: Arc<Mutex<WatcherState>>,
}

#[derive(Default)]
struct EventHub {
    clients: Mutex<Vec<Sender<Value>>>,
    connected_viewers: Mutex<usize>,
    last_viewer_seen: Mutex<Option<Instant>>,
}

impl EventHub {
    fn subscribe(&self) -> Receiver<Value> {
        let (sender, receiver) = mpsc::channel();
        if let Ok(mut last_seen) = self.last_viewer_seen.lock() {
            *last_seen = Some(Instant::now());
        }
        if let Ok(mut clients) = self.clients.lock() {
            clients.push(sender);
        }
        if let Ok(mut connected_viewers) = self.connected_viewers.lock() {
            *connected_viewers += 1;
        }
        receiver
    }

    fn viewer_has_been_seen(&self) -> bool {
        self.last_viewer_seen
            .lock()
            .ok()
            .is_some_and(|last_seen| last_seen.is_some())
    }

    fn viewer_is_connected(&self) -> bool {
        let has_connected_viewer = self
            .connected_viewers
            .lock()
            .is_ok_and(|connected_viewers| *connected_viewers > 0);
        has_connected_viewer
            || self
                .last_viewer_seen
                .lock()
                .ok()
                .and_then(|last_seen| *last_seen)
                .is_some_and(|last_seen| last_seen.elapsed() < Duration::from_secs(6))
    }

    fn should_open_browser(&self, browser_opened: bool) -> bool {
        !browser_opened || (self.viewer_has_been_seen() && !self.viewer_is_connected())
    }

    fn disconnect_viewer(&self) {
        if let Ok(mut connected_viewers) = self.connected_viewers.lock() {
            *connected_viewers = connected_viewers.saturating_sub(1);
        }
    }

    fn publish(&self, payload: Value) {
        let Ok(mut clients) = self.clients.lock() else {
            return;
        };
        clients.retain(|client| client.send(payload.clone()).is_ok());
    }
}

struct WatcherState {
    watcher: RecommendedWatcher,
    roots: HashSet<PathBuf>,
    artifacts: HashSet<PathBuf>,
}

impl WatcherState {
    fn watch_record(&mut self, record: &SessionRecord) -> Result<()> {
        let artifact = record.cwd.join(&record.artifact_path);
        if !artifact.is_file() {
            return Ok(());
        }
        let Some(root) = artifact.parent() else {
            return Ok(());
        };
        if !self.roots.contains(root) {
            self.watcher.watch(root, RecursiveMode::NonRecursive)?;
            self.roots.insert(root.to_path_buf());
        }
        self.artifacts.insert(artifact);
        Ok(())
    }

    fn watch_recent_record(&mut self, record: &SessionRecord) -> Result<()> {
        let artifact = record.cwd.join(&record.artifact_path);
        if storage::file_age(&artifact).is_ok_and(|age| age <= Duration::from_secs(24 * 60 * 60)) {
            self.watch_record(record)?;
        }
        Ok(())
    }

    fn watched_artifacts(&self) -> &HashSet<PathBuf> {
        &self.artifacts
    }
}

impl Daemon {
    pub fn run_foreground(port: u16) -> Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let actual_port = listener.local_addr()?.port();
        let server = Arc::new(Server::from_listener(listener, None)?);
        let registry = storage::load_registry()?;
        let (file_event_sender, file_event_receiver) = mpsc::channel();
        let watcher = RecommendedWatcher::new(
            move |result| {
                let _ = file_event_sender.send(result);
            },
            Config::default(),
        )?;
        let watcher = Arc::new(Mutex::new(WatcherState {
            watcher,
            roots: HashSet::new(),
            artifacts: HashSet::new(),
        }));
        if let Ok(mut watcher_state) = watcher.lock() {
            for record in &registry.sessions {
                watcher_state.watch_recent_record(record)?;
            }
        }
        let events = Arc::new(EventHub {
            clients: Mutex::new(Vec::new()),
            connected_viewers: Mutex::new(0),
            last_viewer_seen: Mutex::new(None),
        });
        let state = Arc::new(Mutex::new(AppState {
            registry,
            browser_opened: false,
            events,
            watcher,
        }));

        let watch_state = Arc::clone(&state);
        thread::spawn(move || watch_files(file_event_receiver, watch_state));

        fs::create_dir_all(storage::app_data_dir())?;
        let info_path = storage::daemon_info_path();
        fs::write(
            &info_path,
            serde_json::to_vec_pretty(&DaemonInfo {
                port: actual_port,
                pid: std::process::id(),
            })?,
        )?;
        let _cleanup = DaemonInfoCleanup { path: info_path };

        for request in server.incoming_requests() {
            let state = Arc::clone(&state);
            let server = Arc::clone(&server);
            thread::spawn(move || {
                if let Err(error) = handle_request(request, state, actual_port, server) {
                    eprintln!("session-whiteboard daemon: {error}");
                }
            });
        }
        Ok(())
    }

    pub fn start_managed() -> Result<DaemonInfo> {
        let _ = ensure_daemon()?;
        read_daemon_info()
    }

    pub fn stop_managed() -> Result<bool> {
        let info_path = storage::daemon_info_path();
        let Ok(info) = read_daemon_info() else {
            return Ok(false);
        };
        if !is_expected_daemon(&info) {
            let stopped =
                client_request_at_port(info.port, "POST", "/api/daemon/stop", &[]).is_ok();
            let _ = fs::remove_file(info_path);
            return Ok(stopped);
        }
        let _ = client_request_at_port(info.port, "POST", "/api/daemon/stop", &[])?;
        for _ in 0..50 {
            if !info_path.exists() || !is_expected_daemon(&info) {
                return Ok(true);
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(true)
    }

    pub fn restart_managed() -> Result<DaemonInfo> {
        let _ = Self::stop_managed()?;
        Self::start_managed()
    }

    pub fn prepare_via_client(request: &SessionRequest) -> Result<PrepareResponse> {
        let body = serde_json::to_vec(request)?;
        let response = client_request("POST", "/api/sessions/prepare", &body)?;
        Ok(serde_json::from_slice(&response)?)
    }

    pub fn browse_via_client() -> Result<BrowseResponse> {
        let response = client_request("POST", "/api/viewer/open", &[])?;
        Ok(serde_json::from_slice(&response)?)
    }

    pub fn clean_via_client(request: &SessionRequest) -> Result<CleanResponse> {
        let body = serde_json::to_vec(request)?;
        let response = client_request("POST", "/api/sessions/clean", &body)?;
        Ok(serde_json::from_slice(&response)?)
    }
}

struct DaemonInfoCleanup {
    path: std::path::PathBuf,
}

impl Drop for DaemonInfoCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn watch_files(receiver: Receiver<notify::Result<Event>>, state: Arc<Mutex<AppState>>) {
    let mut versions = std::collections::HashMap::new();
    while let Ok(result) = receiver.recv() {
        let Ok(event) = result else {
            continue;
        };
        for path in event.paths {
            thread::sleep(Duration::from_millis(30));
            for (key, version, events) in changed_artifacts(&state, &path) {
                if versions.get(&key) == Some(&version) {
                    continue;
                }
                versions.insert(key.clone(), version.clone());
                events.publish(json!({
                    "type": "artifact_updated",
                    "key": key,
                    "version": version,
                }));
            }
        }
    }
}

fn changed_artifacts(
    state: &Arc<Mutex<AppState>>,
    path: &Path,
) -> Vec<(String, String, Arc<EventHub>)> {
    let Ok(state) = state.lock() else {
        return Vec::new();
    };
    let Ok(watcher) = state.watcher.lock() else {
        return Vec::new();
    };
    let watched = watcher.watched_artifacts().clone();
    state
        .registry
        .sessions
        .iter()
        .filter_map(|record| {
            let artifact = record.cwd.join(&record.artifact_path);
            if !watched.contains(&artifact) || (artifact != path && artifact.parent() != Some(path))
            {
                return None;
            }
            let version = storage::file_version(&artifact).ok()?;
            Some((record.key.clone(), version, Arc::clone(&state.events)))
        })
        .collect()
}

fn respond_events(
    request: Request,
    events: Arc<EventHub>,
    receiver: Receiver<Value>,
) -> Result<()> {
    let payload = receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| json!({"type": "heartbeat"}));
    let result = respond_json(request, serde_json::to_vec(&payload)?);
    events.disconnect_viewer();
    result
}

fn handle_request(
    mut request: Request,
    state: Arc<Mutex<AppState>>,
    port: u16,
    server: Arc<Server>,
) -> Result<()> {
    let method = request.method().clone();
    let path = request.url().split('?').next().unwrap_or(request.url());

    match (method, path) {
        (Method::Get, "/") => respond_html(request, include_str!("../web/index.html")),
        (Method::Get, "/health") => {
            let body = serde_json::to_vec(&HealthResponse {
                ok: true,
                pid: std::process::id(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })?;
            respond_json(request, body)
        }
        (Method::Get, "/api/events") => {
            let events = {
                let state = state.lock().map_err(|_| "daemon state lock poisoned")?;
                Arc::clone(&state.events)
            };
            let receiver = events.subscribe();
            respond_events(request, events, receiver)
        }
        (Method::Post, "/api/viewer/open") => {
            let mut state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let opened = open_browser(&viewer_root_url(port));
            if opened {
                state.browser_opened = true;
            }
            respond_json(
                request,
                serde_json::to_vec(&BrowseResponse {
                    viewer_url: viewer_root_url(port),
                    opened,
                })?,
            )
        }
        (Method::Post, "/api/daemon/stop") => {
            let response = respond_json(request, br#"{"ok":true}"#.to_vec());
            server.unblock();
            response
        }
        (Method::Post, "/api/sessions/prepare") => {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body)?;
            let prepare_request: SessionRequest = serde_json::from_str(&body)?;
            let mut state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let (record, warning) =
                storage::prepare_artifact(&mut state.registry, &prepare_request)?;
            storage::save_registry(&state.registry)?;
            if let Ok(mut watcher) = state.watcher.lock()
                && let Err(error) = watcher.watch_record(&record)
            {
                eprintln!("session-whiteboard daemon: could not watch artifact: {error}");
            }
            let title = storage::read_title(&record.cwd.join(&record.artifact_path))
                .unwrap_or_else(|_| "Untitled session".to_string());
            let viewer_url = viewer_url(port, &record.key);
            let should_open_browser = state.events.should_open_browser(state.browser_opened);
            if should_open_browser && open_browser(&viewer_url) {
                state.browser_opened = true;
            }
            let response = PrepareResponse {
                provider: record.provider,
                session_id: record.session_id,
                artifact_path: record.artifact_path,
                relative_to: record.cwd.clone(),
                viewer_url,
                title,
                warning,
            };
            respond_json(request, serde_json::to_vec(&response)?)
        }
        (Method::Post, "/api/sessions/clean") => {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body)?;
            let clean_request: SessionRequest = serde_json::from_str(&body)?;
            let mut state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let cleaned_record = storage::clean_record(&mut state.registry, &clean_request)?;
            storage::save_registry(&state.registry)?;
            let response = CleanResponse {
                provider: clean_request.provider,
                session_id: clean_request.session_id,
                cleaned: cleaned_record.is_some(),
                artifact_path: cleaned_record.map(|record| record.artifact_path),
            };
            respond_json(request, serde_json::to_vec(&response)?)
        }
        (Method::Get, path) if path.starts_with("/api/sessions/") && path.ends_with("/version") => {
            let key = &path["/api/sessions/".len()..path.len() - "/version".len()];
            let state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let Some(record) = storage::find_record(&state.registry, key) else {
                return respond_error(request, 404, "unknown session");
            };
            let version = storage::file_version(&record.cwd.join(&record.artifact_path))?;
            respond_text(request, version)
        }
        (Method::Get, path) if path.starts_with("/session/") => {
            let key = path.trim_start_matches("/session/");
            let state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let Some(record) = storage::find_record(&state.registry, key) else {
                return respond_error(request, 404, "unknown session");
            };
            let record = record.clone();
            if let Ok(mut watcher) = state.watcher.lock()
                && let Err(error) = watcher.watch_record(&record)
            {
                eprintln!("session-whiteboard daemon: could not watch displayed artifact: {error}");
            }
            let bytes = fs::read(record.cwd.join(&record.artifact_path))?;
            respond_bytes(request, bytes, "text/html; charset=utf-8")
        }
        _ => respond_error(request, 404, "not found"),
    }
}

fn respond_json(request: Request, body: Vec<u8>) -> Result<()> {
    respond_bytes(request, body, "application/json; charset=utf-8")
}

fn respond_html(request: Request, body: &str) -> Result<()> {
    respond_bytes(
        request,
        body.as_bytes().to_vec(),
        "text/html; charset=utf-8",
    )
}

fn respond_text(request: Request, body: String) -> Result<()> {
    respond_bytes(request, body.into_bytes(), "text/plain; charset=utf-8")
}

fn respond_bytes(request: Request, body: Vec<u8>, content_type: &str) -> Result<()> {
    let content_type =
        Header::from_bytes("Content-Type", content_type).map_err(|_| "invalid content type")?;
    let cache_control =
        Header::from_bytes("Cache-Control", "no-store").map_err(|_| "invalid cache header")?;
    request.respond(
        Response::from_data(body)
            .with_header(content_type)
            .with_header(cache_control),
    )?;
    Ok(())
}

fn respond_error(request: Request, status: u16, message: &str) -> Result<()> {
    let body = serde_json::json!({ "error": message }).to_string();
    let content_type = Header::from_bytes("Content-Type", "application/json; charset=utf-8")
        .map_err(|_| "invalid content type")?;
    request.respond(
        Response::from_string(body)
            .with_status_code(StatusCode(status))
            .with_header(content_type),
    )?;
    Ok(())
}

fn viewer_url(port: u16, key: &str) -> String {
    format!("http://127.0.0.1:{port}/?session={key}")
}

fn viewer_root_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/")
}

fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let result = Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let result: std::io::Result<std::process::Child> = Err(std::io::Error::other(
        "automatic browser opening is unsupported",
    ));

    match result {
        Ok(_) => true,
        Err(error) => {
            eprintln!("session-whiteboard: could not open browser: {error}");
            false
        }
    }
}

fn client_request(method: &str, path: &str, body: &[u8]) -> Result<Vec<u8>> {
    let port = ensure_daemon()?;
    client_request_at_port(port, method, path, body)
}

fn client_request_at_port(port: u16, method: &str, path: &str, body: &[u8]) -> Result<Vec<u8>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    stream.write_all(body)?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let separator = b"\r\n\r\n";
    let Some(body_start) = response
        .windows(separator.len())
        .position(|window| window == separator)
    else {
        return Err("invalid response from daemon".into());
    };
    let header = String::from_utf8_lossy(&response[..body_start]);
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .unwrap_or(500);
    let body = response[body_start + separator.len()..].to_vec();
    if status >= 400 {
        return Err(format!(
            "daemon returned HTTP {status}: {}",
            String::from_utf8_lossy(&body)
        )
        .into());
    }
    Ok(body)
}

fn ensure_daemon() -> Result<u16> {
    if let Ok(info) = read_daemon_info() {
        if is_expected_daemon(&info) {
            return Ok(info.port);
        }
        let _ = client_request_at_port(info.port, "POST", "/api/daemon/stop", &[]);
        let _ = fs::remove_file(storage::daemon_info_path());
    }

    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("daemon")
        .arg("--port")
        .arg("0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A managed daemon must survive the short-lived CLI process. A new
        // session also keeps terminal/process-tree cleanup from taking it
        // down when the caller exits.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command.spawn()?;

    for _ in 0..50 {
        if let Ok(info) = read_daemon_info()
            && is_expected_daemon(&info)
        {
            return Ok(info.port);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("daemon did not become ready; if this command is running in a sandbox, retry it with the environment's escalation/approval mechanism, or use the host/proxy/MCP execution path so the daemon can bind localhost and write its runtime state".into())
}

fn read_daemon_info() -> Result<DaemonInfo> {
    let content = fs::read_to_string(storage::daemon_info_path())?;
    Ok(serde_json::from_str(&content)?)
}

fn is_expected_daemon(info: &DaemonInfo) -> bool {
    health_response(info.port).is_some_and(|health| {
        health.ok && health.pid == info.pid && health.version == env!("CARGO_PKG_VERSION")
    })
}

fn health_response(port: u16) -> Option<HealthResponse> {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return None;
    };
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return None;
    }
    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok()?;
    let separator = b"\r\n\r\n";
    let body_start = response
        .windows(separator.len())
        .position(|window| window == separator)?;
    let header = String::from_utf8_lossy(&response[..body_start]);
    if !header.lines().next()?.contains("200 OK") {
        return None;
    }
    serde_json::from_slice(&response[body_start + separator.len()..]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_url_points_to_one_session_board() {
        assert_eq!(
            viewer_url(43123, "codex-abc123"),
            "http://127.0.0.1:43123/?session=codex-abc123"
        );
    }

    #[test]
    fn browser_open_policy_reopens_only_after_a_known_viewer_disconnects() {
        let hub = EventHub::default();
        assert!(hub.should_open_browser(false));
        assert!(!hub.should_open_browser(true));

        let _receiver = hub.subscribe();
        assert!(!hub.should_open_browser(true));
        hub.disconnect_viewer();
        if let Ok(mut last_seen) = hub.last_viewer_seen.lock() {
            *last_seen = Some(Instant::now() - Duration::from_secs(7));
        }
        assert!(hub.should_open_browser(true));
    }
}
