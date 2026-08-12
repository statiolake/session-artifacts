use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;

use crate::model::{OpenRequest, Provider, Registry, SessionRecord, SessionSummary};
use crate::template;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const ARTIFACT_ROOT: &str = ".session-whiteboard";
const LEGACY_ARTIFACT_ROOT: &str = ".session-artifacts";

pub fn app_data_dir() -> PathBuf {
    ProjectDirs::from("", "", "session-whiteboard")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("session-whiteboard"))
}

fn legacy_app_data_dir() -> PathBuf {
    ProjectDirs::from("", "", "session-artifacts")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("session-artifacts"))
}

pub fn daemon_info_path() -> PathBuf {
    app_data_dir().join("daemon.json")
}

pub fn registry_path() -> PathBuf {
    app_data_dir().join("registry.json")
}

pub fn load_registry() -> Result<Registry> {
    let path = registry_path();
    let path = if path.exists() {
        path
    } else {
        let legacy_path = legacy_app_data_dir().join("registry.json");
        if legacy_path.exists() {
            legacy_path
        } else {
            return Ok(Registry::default());
        }
    };
    if !path.exists() {
        return Ok(Registry::default());
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

pub fn save_registry(registry: &Registry) -> Result<()> {
    let path = registry_path();
    fs::create_dir_all(path.parent().expect("registry has a parent"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(registry)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub fn session_key(provider: &Provider, session_id: &str, cwd: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    provider.as_str().hash(&mut hasher);
    session_id.hash(&mut hasher);
    cwd.hash(&mut hasher);
    format!("{}-{:016x}", provider.as_str(), hasher.finish())
}

pub fn artifact_relative_path(provider: &Provider, key: &str) -> PathBuf {
    PathBuf::from(ARTIFACT_ROOT)
        .join(provider.as_str())
        .join(format!("{key}.html"))
}

fn legacy_artifact_relative_path(provider: &Provider, key: &str) -> PathBuf {
    PathBuf::from(LEGACY_ARTIFACT_ROOT)
        .join(provider.as_str())
        .join(format!("{key}.html"))
}

fn migrate_legacy_artifact_path(cwd: &Path, relative_path: &Path) -> Result<PathBuf> {
    let Some(suffix) = relative_path.strip_prefix(LEGACY_ARTIFACT_ROOT).ok() else {
        return Ok(relative_path.to_path_buf());
    };
    let new_relative_path = PathBuf::from(ARTIFACT_ROOT).join(suffix);
    let old_absolute_path = cwd.join(relative_path);
    let new_absolute_path = cwd.join(&new_relative_path);
    if old_absolute_path.exists() && !new_absolute_path.exists() {
        if let Some(parent) = new_absolute_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(old_absolute_path, &new_absolute_path)?;
    }
    Ok(new_relative_path)
}

pub fn prepare_artifact(
    registry: &mut Registry,
    request: &OpenRequest,
) -> Result<(SessionRecord, Option<String>)> {
    if request.session_id.trim().is_empty() {
        return Err("session_id must not be empty".into());
    }
    if !request.cwd.is_absolute() {
        return Err("cwd must be absolute".into());
    }

    let key = session_key(&request.provider, &request.session_id, &request.cwd);
    let timestamp = now();
    let mut warning = None;

    let record = if let Some(record) = registry.sessions.iter_mut().find(|record| {
        record.provider == request.provider
            && record.session_id == request.session_id
            && record.cwd == request.cwd
    }) {
        record.artifact_path = migrate_legacy_artifact_path(&request.cwd, &record.artifact_path)?;
        record.active = true;
        record.updated_at = timestamp;
        record.clone()
    } else {
        let relative_path = artifact_relative_path(&request.provider, &key);
        let relative_path = if request.cwd.join(&relative_path).exists() {
            relative_path
        } else {
            migrate_legacy_artifact_path(
                &request.cwd,
                &legacy_artifact_relative_path(&request.provider, &key),
            )?
        };
        let absolute_path = request.cwd.join(&relative_path);
        if !absolute_path.exists() {
            if let Some(parent) = absolute_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                &absolute_path,
                template::render_session_template("New session"),
            )?;
        }
        let record = SessionRecord {
            key,
            provider: request.provider.clone(),
            session_id: request.session_id.clone(),
            cwd: request.cwd.clone(),
            artifact_path: relative_path,
            active: true,
            created_at: timestamp,
            updated_at: timestamp,
        };
        registry.sessions.push(record.clone());
        record
    };

    if let Err(error) = ensure_git_exclude(&request.cwd) {
        warning = Some(error);
    }

    Ok((record, warning))
}

pub fn mark_closed(registry: &mut Registry, request: &OpenRequest) -> bool {
    if let Some(record) = registry.sessions.iter_mut().find(|record| {
        record.provider == request.provider
            && record.session_id == request.session_id
            && record.cwd == request.cwd
    }) {
        record.active = false;
        record.updated_at = now();
        true
    } else {
        false
    }
}

pub fn delete_record(
    registry: &mut Registry,
    request: &OpenRequest,
) -> Result<Option<SessionRecord>> {
    let Some(index) = registry.sessions.iter().position(|record| {
        record.provider == request.provider
            && record.session_id == request.session_id
            && record.cwd == request.cwd
    }) else {
        return Ok(None);
    };
    let record = registry.sessions[index].clone();
    let absolute_path = record.cwd.join(&record.artifact_path);
    if fs::symlink_metadata(&absolute_path).is_ok() {
        fs::remove_file(absolute_path)?;
    }
    registry.sessions.remove(index);
    Ok(Some(record))
}

pub fn active_summaries(registry: &Registry) -> Vec<SessionSummary> {
    registry
        .sessions
        .iter()
        .filter(|record| record.active)
        .filter_map(|record| summarize(record).ok())
        .collect()
}

pub fn find_record<'a>(registry: &'a Registry, key: &str) -> Option<&'a SessionRecord> {
    registry.sessions.iter().find(|record| record.key == key)
}

pub fn summarize(record: &SessionRecord) -> Result<SessionSummary> {
    let absolute_path = record.cwd.join(&record.artifact_path);
    let title = read_title(&absolute_path).unwrap_or_else(|_| "Untitled session".to_string());
    Ok(SessionSummary {
        key: record.key.clone(),
        provider: record.provider.clone(),
        session_id: record.session_id.clone(),
        cwd: record.cwd.clone(),
        artifact_path: record.artifact_path.clone(),
        title,
        active: record.active,
        updated_at: record.updated_at,
        version: file_version(&absolute_path).unwrap_or_else(|_| "missing".to_string()),
    })
}

pub fn file_version(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(format!("{modified}-{}", metadata.len()))
}

pub fn read_title(path: &Path) -> Result<String> {
    let content = strip_html_comments(&fs::read_to_string(path)?);
    let lower = content.to_ascii_lowercase();
    let title_tag = "<title>";
    let content_start = lower
        .find(title_tag)
        .map(|start| start + title_tag.len())
        .ok_or("HTML title element not found")?;
    let end = lower[content_start..]
        .find("</title>")
        .ok_or("HTML title closing tag is missing")?
        + content_start;
    let title = content[content_start..end].trim();
    if title.is_empty() {
        return Ok("Untitled session".to_string());
    }
    Ok(unescape_html(title))
}

fn strip_html_comments(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut remainder = value;
    while let Some(start) = remainder.find("<!--") {
        result.push_str(&remainder[..start]);
        let after_start = &remainder[start + 4..];
        let Some(end) = after_start.find("-->") else {
            break;
        };
        remainder = &after_start[end + 3..];
    }
    result.push_str(remainder);
    result
}

fn unescape_html(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn ensure_git_exclude(cwd: &Path) -> std::result::Result<(), String> {
    let root_output = Command::new("git")
        .current_dir(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("could not check Git repository: {error}"))?;
    if !root_output.status.success() {
        return Ok(());
    }
    let root = PathBuf::from(String::from_utf8_lossy(&root_output.stdout).trim());
    let root =
        fs::canonicalize(root).map_err(|error| format!("could not resolve Git root: {error}"))?;

    let git_dir_output = Command::new("git")
        .current_dir(cwd)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map_err(|error| format!("could not locate Git metadata: {error}"))?;
    if !git_dir_output.status.success() {
        return Ok(());
    }
    let git_dir = PathBuf::from(String::from_utf8_lossy(&git_dir_output.stdout).trim());
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        root.join(git_dir)
    };
    let exclude_path = git_dir.join("info").join("exclude");
    let artifact_root = cwd.join(ARTIFACT_ROOT);
    let relative = artifact_root
        .strip_prefix(&root)
        .map_err(|error| format!("could not make artifact path relative to Git root: {error}"))?;
    let pattern = format!("/{}/", path_to_slashes(relative));
    let legacy_artifact_root = cwd.join(LEGACY_ARTIFACT_ROOT);
    let legacy_relative = legacy_artifact_root.strip_prefix(&root).map_err(|error| {
        format!("could not make legacy artifact path relative to Git root: {error}")
    })?;
    let legacy_pattern = format!("/{}/", path_to_slashes(legacy_relative));

    let original_content = if exclude_path.exists() {
        fs::read_to_string(&exclude_path)
            .map_err(|error| format!("could not read {}: {error}", exclude_path.display()))?
    } else {
        String::new()
    };
    let has_pattern = original_content.lines().any(|line| line.trim() == pattern);
    let had_legacy_pattern = original_content
        .lines()
        .any(|line| line.trim() == legacy_pattern);
    if has_pattern && !had_legacy_pattern {
        return Ok(());
    }
    let mut content = original_content
        .lines()
        .filter(|line| line.trim() != legacy_pattern)
        .collect::<Vec<_>>()
        .join("\n");
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    if !has_pattern {
        content.push_str(&pattern);
        content.push('\n');
    }

    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create Git exclude directory: {error}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&exclude_path)
        .map_err(|error| format!("could not write {}: {error}", exclude_path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("could not update {}: {error}", exclude_path.display()))?;
    Ok(())
}

fn path_to_slashes(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_reader_ignores_comments_before_title() {
        let path = std::env::temp_dir().join(format!(
            "session-whiteboard-title-test-{}-{}.html",
            std::process::id(),
            now()
        ));
        fs::write(
            &path,
            "<!-- <title>comment text</title> --><title>Actual &amp; title</title>",
        )
        .expect("write test HTML");
        assert_eq!(read_title(&path).expect("read title"), "Actual & title");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn artifact_paths_are_relative_to_the_session_cwd() {
        let path = artifact_relative_path(&Provider::Codex, "codex-demo-1234");
        assert_eq!(
            path,
            PathBuf::from(".session-whiteboard/codex/codex-demo-1234.html")
        );
    }

    #[test]
    fn opening_a_legacy_artifact_moves_it_to_the_whiteboard_root() {
        let cwd = std::env::temp_dir().join(format!(
            "session-whiteboard-migration-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cwd);
        let request = OpenRequest {
            provider: Provider::Codex,
            session_id: "legacy-session".to_string(),
            cwd: cwd.clone(),
        };
        let key = session_key(&request.provider, &request.session_id, &request.cwd);
        let legacy_path = legacy_artifact_relative_path(&request.provider, &key);
        let legacy_absolute_path = cwd.join(&legacy_path);
        fs::create_dir_all(legacy_absolute_path.parent().expect("legacy parent"))
            .expect("create legacy artifact parent");
        fs::write(&legacy_absolute_path, "legacy whiteboard").expect("write legacy whiteboard");

        let mut registry = Registry::default();
        let (record, warning) =
            prepare_artifact(&mut registry, &request).expect("migrate artifact");
        assert!(warning.is_none());
        assert_eq!(
            record.artifact_path,
            artifact_relative_path(&request.provider, &key)
        );
        assert_eq!(
            fs::read_to_string(cwd.join(&record.artifact_path)).expect("read migrated whiteboard"),
            "legacy whiteboard"
        );
        assert!(!legacy_absolute_path.exists());

        let _ = fs::remove_dir_all(cwd);
    }

    #[test]
    fn delete_removes_the_explicit_artifact_and_registry_record() {
        let cwd = std::env::temp_dir().join(format!(
            "session-whiteboard-delete-test-{}",
            std::process::id()
        ));
        let artifact_path = PathBuf::from(".session-whiteboard/codex/delete-test.html");
        let absolute_path = cwd.join(&artifact_path);
        fs::create_dir_all(absolute_path.parent().expect("artifact parent"))
            .expect("create artifact parent");
        fs::write(&absolute_path, "<!doctype html>").expect("write artifact");
        let request = OpenRequest {
            provider: Provider::Codex,
            session_id: "delete-test".to_string(),
            cwd: cwd.clone(),
        };
        let mut registry = Registry {
            sessions: vec![SessionRecord {
                key: "delete-test-key".to_string(),
                provider: Provider::Codex,
                session_id: "delete-test".to_string(),
                cwd: cwd.clone(),
                artifact_path,
                active: false,
                created_at: now(),
                updated_at: now(),
            }],
        };
        let deleted = delete_record(&mut registry, &request)
            .expect("delete record")
            .expect("record exists");
        assert_eq!(deleted.session_id, "delete-test");
        assert!(registry.sessions.is_empty());
        assert!(!absolute_path.exists());
        let _ = fs::remove_dir_all(cwd);
    }
}
