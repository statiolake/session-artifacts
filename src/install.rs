use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::model::Provider;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const PRODUCT_NAME: &str = "session-whiteboard";
const LEGACY_PRODUCT_NAME: &str = "session-artifacts";
const HOOK_MARKER: &str = "session-whiteboard-hook-v3";
const LEGACY_HOOK_MARKER: &str = "session-artifacts-hook-v2";

#[derive(Debug, serde::Serialize)]
pub struct InstallResult {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct UninstallResult {
    pub removed: Vec<String>,
    pub skipped: Vec<String>,
    pub notes: Vec<String>,
}

pub fn skill_text(provider: Provider) -> String {
    let provider_name = provider.as_str();
    format!(
        r#"---
name: session-whiteboard
description: Use one live HTML whiteboard when the user explicitly asks for a whiteboard explanation or when a compact two-dimensional explanation would materially improve clarity. It may be used often, but it is not required on every turn.
---

# Session Whiteboard

This is an opt-in companion to the conversation. Use it when the user explicitly
asks for a whiteboard, for example "ホワイトボードで説明して" or
"ちょっとそこホワイトボードにまとめて", and also when you judge that a
compact two-dimensional explanation would materially improve understanding. It
is fine to use it frequently, including on consecutive turns, but do not treat
it as a required per-turn log or update it merely because a turn ended. Keep
the normal answer in chat; the whiteboard adds a spatial explanation when it is
useful.

Prefer the board when the explanation has several entities, branches,
dependencies, comparisons, timelines, layers, or constraints whose
relationships are easier to see spatially. Keep simple questions, short code
edits, and linear answers in chat. When the benefit is marginal, stay in chat.
The board is a disposable current-context projection, not a transcript, archive,
or complete knowledge base.

The board is for an engineer explaining a technical subject to another
engineer. Text is not the goal by itself: use position, grouping, connectors,
alignment, and short labels to make relationships visible in two dimensions.
Keep the page practical and information-dense. Do not make a slogan, manifesto,
or oversized hero heading out of the subject.

## When the board is useful

1. Obtain the current provider, session ID, and cwd from the agent context. Do
   not invent a session ID. If the session ID is unavailable, ask for it rather
   than silently creating a board that cannot be resumed.
2. Obtain the artifact path through the execution mechanism that works in the
   current environment:

       session-whiteboard prepare --provider {provider_name} --session-id <id> --cwd <cwd> --json

   If a direct executable is unavailable, use the appropriate host, proxy, MCP,
   or other configured mechanism.
3. Use the returned artifact_path as a path relative to relative_to. Do not
   create a replacement file in the repository and do not choose another path.
4. Re-render the complete HTML document from the current explanation and replace
   the existing file. Do not append a new log entry or preserve stale content
   just for completeness. The previous whiteboard is a disposable draft.
5. Give the page a concise title that names the subject being explained. The
   <title> names this board in the browser document, so do not use a sentence or
   a generic slogan.
   Keep the visible heading synchronized with it.
6. Include only the information needed to understand the current explanation:
   for example a question or claim, relevant entities and relationships,
   evidence or constraints, decisions, and the next useful action. Choose the
   sections for the subject; do not force a fixed checklist or retain solved
   branches.
7. Use spatial structure deliberately. Let grouping, relative position,
   connectors, emphasis, and compact annotations carry meaning that a linear
   paragraph would obscure. Use normal readable type; the board is not a poster
   with one oversized headline.
8. Keep the HTML self-contained. Inline CSS, JavaScript, SVG, and small data
   are preferred. External links are allowed when useful, including local file
   links.
9. When referring to a local source file, include the visible relative/path:line
   as a copy action that writes that exact string to the clipboard. Do not use
   an editor-specific URL scheme. Keep the readable path and line visible if
   the Clipboard API is not available.
10. Keep the normal board compact and highly scannable. If secondary context does not fit, collapse it
   behind an explicit click target such as <details> or a dialog/popover and label
   what is hidden. If essential context still needs more room, let the document
   scroll; never allow cards to overlap or become unreadable, and do not turn the
   board into a long scrolling transcript.
11. Read .interface-design/system.md when it is present and follow its paper
   whiteboard contract: 4px spacing rhythm, warm paper surface, graphite rules,
   one blue marker accent, a broad explanation field with narrow marginalia,
   and no generic equal-card dashboard grid.
12. Keep the ordinary chat answer useful as well. The whiteboard supplements the
   conversation; it does not replace it with a status-only message.

## Viewer and cleanup

The returned viewer_url displays this one whiteboard full-screen. There is no
sidebar or session switcher. `session-whiteboard browse` opens the daemon's
empty landing page; prefer the viewer_url returned by `prepare` for a board.

The registry is a known-board index, not a session lifecycle state machine. The
viewer serves one board per URL. Use `session-whiteboard clean` only when the
HTML and its registry entry should be permanently deleted.

If the whiteboard command or file edit is genuinely unavailable after
reasonable attempts, answer normally in chat and explain that the whiteboard
update failed. Do not silently lose a substantive answer.

Provider for this skill: {provider_name}.
"#
    )
}

pub fn install(provider: Option<Provider>) -> Result<InstallResult> {
    let providers = provider
        .map(|provider| vec![provider])
        .unwrap_or_else(|| vec![Provider::Claude, Provider::Codex]);
    let mut result = InstallResult {
        installed: Vec::new(),
        skipped: Vec::new(),
        notes: Vec::new(),
    };
    for provider in providers {
        match provider {
            Provider::Claude => install_claude(&mut result)?,
            Provider::Codex => install_codex(&mut result)?,
            Provider::Generic => {
                result
                    .skipped
                    .push("generic provider has no global installation target".to_string());
            }
        }
    }
    result.notes.push(
        "Installed the opt-in skill only. Automatic session-whiteboard hooks are not installed; existing matching hooks were removed.".to_string(),
    );
    Ok(result)
}

pub fn uninstall(provider: Option<Provider>) -> Result<UninstallResult> {
    let providers = provider
        .map(|provider| vec![provider])
        .unwrap_or_else(|| vec![Provider::Claude, Provider::Codex]);
    let mut result = UninstallResult {
        removed: Vec::new(),
        skipped: Vec::new(),
        notes: Vec::new(),
    };
    for provider in providers {
        match provider {
            Provider::Claude => uninstall_claude(&mut result)?,
            Provider::Codex => uninstall_codex(&mut result)?,
            Provider::Generic => {
                result
                    .skipped
                    .push("generic provider has no global installation target".to_string());
            }
        }
    }
    result.notes.push(
        "Only session-whiteboard hook entries were removed; unrelated provider settings were preserved."
            .to_string(),
    );
    Ok(result)
}

fn install_claude(result: &mut InstallResult) -> Result<()> {
    let home = home_dir()?;
    remove_legacy_skill_files(&home, "claude", &mut result.notes)?;
    let skill_dir = home
        .join(".claude")
        .join("skills")
        .join("session-whiteboard");
    write_if_changed(&skill_dir.join("SKILL.md"), &skill_text(Provider::Claude))?;
    result
        .installed
        .push(skill_dir.join("SKILL.md").display().to_string());

    let settings_path = home.join(".claude").join("settings.json");
    if remove_provider_hooks(&settings_path, &Provider::Claude)? {
        result.notes.push(format!(
            "Removed existing session-whiteboard hooks from {}.",
            settings_path.display()
        ));
    }
    Ok(())
}

fn uninstall_claude(result: &mut UninstallResult) -> Result<()> {
    let home = home_dir()?;
    for product in [PRODUCT_NAME, LEGACY_PRODUCT_NAME] {
        let skill_path = home
            .join(".claude")
            .join("skills")
            .join(product)
            .join("SKILL.md");
        remove_skill_file(&skill_path, result)?;
    }

    let settings_path = home.join(".claude").join("settings.json");
    if remove_provider_hooks(&settings_path, &Provider::Claude)? {
        result.removed.push(settings_path.display().to_string());
    }
    Ok(())
}

fn install_codex(result: &mut InstallResult) -> Result<()> {
    let home = home_dir()?;
    remove_legacy_skill_files(&home, "codex", &mut result.notes)?;
    let skill_dir = home
        .join(".codex")
        .join("skills")
        .join("session-whiteboard");
    write_if_changed(&skill_dir.join("SKILL.md"), &skill_text(Provider::Codex))?;
    result
        .installed
        .push(skill_dir.join("SKILL.md").display().to_string());

    let hook_template = skill_dir.join("HOOK-CONTEXT.md");
    remove_obsolete_file(&hook_template, &mut result.notes)?;

    let hooks_path = home.join(".codex").join("hooks.json");
    if remove_provider_hooks(&hooks_path, &Provider::Codex)? {
        result.notes.push(format!(
            "Removed existing session-whiteboard hooks from {}.",
            hooks_path.display()
        ));
    }
    Ok(())
}

fn uninstall_codex(result: &mut UninstallResult) -> Result<()> {
    let home = home_dir()?;
    for product in [PRODUCT_NAME, LEGACY_PRODUCT_NAME] {
        let skill_dir = home.join(".codex").join("skills").join(product);
        remove_skill_file(&skill_dir.join("SKILL.md"), result)?;
        remove_skill_file(&skill_dir.join("HOOK-CONTEXT.md"), result)?;
    }

    let hooks_path = home.join(".codex").join("hooks.json");
    if remove_provider_hooks(&hooks_path, &Provider::Codex)? {
        result.removed.push(hooks_path.display().to_string());
    }
    result.notes.push(
        "Codex hook feature flags were left unchanged so other Codex hooks are not disabled."
            .to_string(),
    );
    Ok(())
}

fn remove_matching_hooks(groups: &mut Vec<Value>, provider: &Provider) -> Result<bool> {
    let mut remaining_groups = Vec::with_capacity(groups.len());
    let mut changed = false;

    for mut group in groups.drain(..) {
        let mut group_changed = false;
        let remove_group = if let Some(group_object) = group.as_object_mut() {
            if let Some(group_hooks_value) = group_object.get_mut("hooks") {
                let group_hooks = group_hooks_value
                    .as_array_mut()
                    .ok_or("hook group hooks must be an array")?;
                let mut remaining_hooks = Vec::with_capacity(group_hooks.len());
                for hook in group_hooks.drain(..) {
                    if is_session_whiteboard_hook(&hook, provider) {
                        group_changed = true;
                        changed = true;
                    } else {
                        remaining_hooks.push(hook);
                    }
                }
                *group_hooks = remaining_hooks;
                group_changed && group_hooks.is_empty()
            } else {
                false
            }
        } else {
            false
        };
        if !remove_group {
            remaining_groups.push(group);
        }
    }
    *groups = remaining_groups;
    Ok(changed)
}

fn remove_skill_file(path: &Path, result: &mut UninstallResult) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path)?;
    result.removed.push(path.display().to_string());
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
    Ok(())
}

fn remove_obsolete_file(path: &Path, notes: &mut Vec<String>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path)?;
    notes.push(format!("Removed obsolete {}.", path.display()));
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
    Ok(())
}

fn remove_legacy_skill_files(
    home: &Path,
    provider_directory: &str,
    notes: &mut Vec<String>,
) -> Result<()> {
    let skill_dir = home
        .join(format!(".{provider_directory}"))
        .join("skills")
        .join(LEGACY_PRODUCT_NAME);
    let mut removed = false;
    for filename in ["SKILL.md", "HOOK-CONTEXT.md"] {
        let path = skill_dir.join(filename);
        if path.exists() {
            fs::remove_file(path)?;
            removed = true;
        }
    }
    if removed {
        let _ = fs::remove_dir(&skill_dir);
        notes.push(format!(
            "Removed the legacy {LEGACY_PRODUCT_NAME} {provider_directory} skill during migration."
        ));
    }
    Ok(())
}

fn remove_provider_hooks(path: &Path, provider: &Provider) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let mut settings: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let before = settings.clone();
    let root = settings
        .as_object_mut()
        .ok_or("hook settings root must be a JSON object")?;
    let Some(hooks_value) = root.get_mut("hooks") else {
        return Ok(false);
    };
    let hooks = hooks_value
        .as_object_mut()
        .ok_or("hook settings hooks must be a JSON object")?;
    let event_names: Vec<String> = hooks.keys().cloned().collect();
    for event_name in event_names {
        let Some(groups_value) = hooks.get_mut(&event_name) else {
            continue;
        };
        let groups = groups_value
            .as_array_mut()
            .ok_or("hook event must be an array")?;
        let event_changed = remove_matching_hooks(groups, provider)?;
        if event_changed && groups.is_empty() {
            hooks.remove(&event_name);
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
    let changed = settings != before;
    if changed {
        write_if_changed(path, &serde_json::to_string_pretty(&settings)?)?;
    }
    Ok(changed)
}

fn is_session_whiteboard_hook(value: &Value, provider: &Provider) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("type").and_then(Value::as_str) != Some("command") {
        return false;
    }
    let Some(command) = object.get("command").and_then(Value::as_str) else {
        return false;
    };
    let marker = format!("[{HOOK_MARKER} provider={}", provider.as_str());
    let legacy_marker = format!("[{LEGACY_HOOK_MARKER} provider={}", provider.as_str());
    if command.contains(&marker) || command.contains(&legacy_marker) {
        return true;
    }
    let suffixes = [
        format!(" hook --provider {}", provider.as_str()),
        format!(" session-end --provider {}", provider.as_str()),
    ];
    let Some(executable) = suffixes
        .iter()
        .find_map(|suffix| command.strip_suffix(suffix).map(str::trim))
    else {
        return false;
    };
    let executable = executable
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .or_else(|| {
            executable
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
        .unwrap_or(executable);
    matches!(
        executable.rsplit(['/', '\\']).next(),
        Some(
            "session-whiteboard" | "session_whiteboard" | "session-artifacts" | "session_artifacts"
        )
    )
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if path.exists() && fs::read_to_string(path)? == content {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn skill_has_yaml_frontmatter() {
        let skill = skill_text(Provider::Codex);
        assert!(skill.starts_with("---\n"));
        assert!(skill.contains("name: session-whiteboard\n"));
        assert!(
            skill.contains(
                "description: Use one live HTML whiteboard when the user explicitly asks"
            )
        );
        assert!(skill.contains("ホワイトボードで説明して"));
        assert!(skill.contains("is fine to use it frequently"));
        assert!(skill.contains("a required per-turn log"));
    }

    #[test]
    fn uninstall_removes_only_session_whiteboard_hook_entries() {
        let path = std::env::temp_dir().join(format!(
            "session-whiteboard-selective-uninstall-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let settings = json!({
            "other_setting": true,
            "hooks": {
                "SessionStart": [
                    {
                        "matcher": "startup",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "'/tmp/session-whiteboard' hook --provider codex"
                            },
                            {
                                "type": "command",
                                "command": "other-hook --provider codex"
                            }
                        ]
                    },
                    {
                        "matcher": "empty-but-unrelated",
                        "hooks": []
                    }
                ],
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "'/tmp/session-whiteboard' hook --provider codex"
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "[session-whiteboard-hook-v3 provider=codex event=Stop] legacy automatic hook"
                            },
                            {
                                "type": "command",
                                "command": "other-tool session-artifacts hook --provider codex"
                            }
                        ]
                    }
                ],
                "Notification": []
            }
        });
        fs::write(
            &path,
            serde_json::to_string_pretty(&settings).expect("serialize hooks"),
        )
        .expect("write hooks");

        assert!(remove_provider_hooks(&path, &Provider::Codex).expect("remove hooks"));
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).expect("read hooks"))
            .expect("valid hooks JSON");
        assert_eq!(settings["other_setting"], true);
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            "other-hook --provider codex"
        );
        assert_eq!(
            settings["hooks"]["SessionStart"].as_array().unwrap().len(),
            2
        );
        assert!(settings["hooks"].get("UserPromptSubmit").is_none());
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"][0]["command"],
            "other-tool session-artifacts hook --provider codex"
        );
        assert!(settings["hooks"].get("Notification").unwrap().is_array());
        assert!(!remove_provider_hooks(&path, &Provider::Codex).expect("second removal"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn uninstall_removes_a_modified_skill_file() {
        let directory = std::env::temp_dir().join(format!(
            "session-whiteboard-skill-uninstall-test-{}",
            std::process::id()
        ));
        let path = directory.join("SKILL.md");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create skill directory");
        fs::write(&path, "user-edited skill").expect("write modified skill");
        let mut result = UninstallResult {
            removed: Vec::new(),
            skipped: Vec::new(),
            notes: Vec::new(),
        };
        remove_skill_file(&path, &mut result).expect("remove modified skill");
        assert!(!path.exists());
        assert_eq!(result.removed, vec![path.display().to_string()]);
        let _ = fs::remove_dir_all(directory);
    }
}
