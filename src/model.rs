use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Codex,
    Claude,
    Generic,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Generic => "generic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRequest {
    pub provider: Provider,
    pub session_id: String,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenResponse {
    pub provider: Provider,
    pub session_id: String,
    pub artifact_path: PathBuf,
    pub relative_to: PathBuf,
    pub viewer_url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseResponse {
    pub provider: Provider,
    pub session_id: String,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResponse {
    pub provider: Provider,
    pub session_id: String,
    pub deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub key: String,
    pub provider: Provider,
    pub session_id: String,
    pub cwd: PathBuf,
    pub artifact_path: PathBuf,
    pub active: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    pub sessions: Vec<SessionRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub key: String,
    pub provider: Provider,
    pub session_id: String,
    pub cwd: PathBuf,
    pub artifact_path: PathBuf,
    pub title: String,
    pub active: bool,
    pub updated_at: u64,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub port: u16,
    pub pid: u32,
}
