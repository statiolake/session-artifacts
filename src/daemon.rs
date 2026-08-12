use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::model::{
    CloseResponse, DaemonInfo, DeleteResponse, HealthResponse, OpenRequest, OpenResponse, Registry,
};
use crate::storage;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct Daemon;

struct AppState {
    registry: Registry,
    browser_opened: HashSet<String>,
}

impl Daemon {
    pub fn run_foreground(port: u16) -> Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let actual_port = listener.local_addr()?.port();
        let server = Arc::new(Server::from_listener(listener, None)?);
        let state = Arc::new(Mutex::new(AppState {
            registry: storage::load_registry()?,
            browser_opened: HashSet::new(),
        }));

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
            let _ = fs::remove_file(info_path);
            return Ok(false);
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

    pub fn open_via_client(request: &OpenRequest) -> Result<OpenResponse> {
        let body = serde_json::to_vec(request)?;
        let response = client_request("POST", "/api/sessions/open", &body)?;
        Ok(serde_json::from_slice(&response)?)
    }

    pub fn close_via_client(request: &OpenRequest) -> Result<CloseResponse> {
        let body = serde_json::to_vec(request)?;
        let response = client_request("POST", "/api/sessions/close", &body)?;
        Ok(serde_json::from_slice(&response)?)
    }

    pub fn delete_via_client(request: &OpenRequest) -> Result<DeleteResponse> {
        let body = serde_json::to_vec(request)?;
        let response = client_request("POST", "/api/sessions/delete", &body)?;
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
            })?;
            respond_json(request, body)
        }
        (Method::Post, "/api/daemon/stop") => {
            let response = respond_json(request, br#"{"ok":true}"#.to_vec());
            server.unblock();
            response
        }
        (Method::Get, "/api/sessions") => {
            let state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            respond_json(
                request,
                serde_json::to_vec(&storage::session_summaries(&state.registry))?,
            )
        }
        (Method::Post, "/api/sessions/open") => {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body)?;
            let open_request: OpenRequest = serde_json::from_str(&body)?;
            let mut state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let (record, warning) = storage::prepare_artifact(&mut state.registry, &open_request)?;
            storage::save_registry(&state.registry)?;
            let title = storage::read_title(&record.cwd.join(&record.artifact_path))
                .unwrap_or_else(|_| "Untitled session".to_string());
            let should_open_browser = state.browser_opened.insert(record.key.clone());
            let viewer_url = viewer_url(port, &record.key);
            if should_open_browser {
                open_browser(&viewer_url);
            }
            let response = OpenResponse {
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
        (Method::Post, "/api/sessions/close") => {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body)?;
            let close_request: OpenRequest = serde_json::from_str(&body)?;
            let mut state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let closed = storage::mark_closed(&mut state.registry, &close_request);
            storage::save_registry(&state.registry)?;
            let response = CloseResponse {
                provider: close_request.provider,
                session_id: close_request.session_id,
                closed,
            };
            respond_json(request, serde_json::to_vec(&response)?)
        }
        (Method::Post, "/api/sessions/delete") => {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body)?;
            let delete_request: OpenRequest = serde_json::from_str(&body)?;
            let mut state = state.lock().map_err(|_| "daemon state lock poisoned")?;
            let deleted_record = storage::delete_record(&mut state.registry, &delete_request)?;
            storage::save_registry(&state.registry)?;
            let response = DeleteResponse {
                provider: delete_request.provider,
                session_id: delete_request.session_id,
                deleted: deleted_record.is_some(),
                artifact_path: deleted_record.map(|record| record.artifact_path),
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

fn open_browser(url: &str) {
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

    if let Err(error) = result {
        eprintln!("session-whiteboard: could not open browser: {error}");
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
    Err("daemon did not become ready".into())
}

fn read_daemon_info() -> Result<DaemonInfo> {
    let content = fs::read_to_string(storage::daemon_info_path())?;
    Ok(serde_json::from_str(&content)?)
}

fn is_expected_daemon(info: &DaemonInfo) -> bool {
    health_response(info.port).is_some_and(|health| health.ok && health.pid == info.pid)
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
    fn viewer_url_points_to_navigation_shell_and_selects_session() {
        assert_eq!(
            viewer_url(43123, "codex-abc123"),
            "http://127.0.0.1:43123/?session=codex-abc123"
        );
    }
}
