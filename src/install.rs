use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::model::Provider;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

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
name: session-artifacts
description: Maintain one live structured HTML artifact as the primary user-facing view of an agent session.
---

# Session Artifacts

Use the session artifact as the primary user-facing view for this session.
The artifact is one self-contained HTML file under the session working
directory. The chat response is only a short transport/status message after a
successful update.

## Required workflow

1. Read the session-artifact context injected by the provider hook. It contains
   the provider, the current session ID, the session cwd, and the command for
   obtaining the artifact path.
2. Run that command through the execution mechanism that works in the current
   environment. The hook only gives instructions; it does not create the file.
   If the first mechanism fails, inspect the available environment and try the
   appropriate configured mechanism before giving up.
3. Use the returned artifact_path as a path relative to relative_to. Do not
   create a replacement file in the repository and do not choose another path.
4. Read the existing HTML, then edit it with the normal file-editing tools.
   Keep one HTML file per session and update the existing structure in place.
5. Put substantive answers, questions, findings, decisions, and next actions
   in the HTML. Do not stream the substantive answer only into chat.
6. Keep the HTML self-contained. Inline CSS, JavaScript, SVG, and small data
   are preferred. External links are allowed when useful, including local file
   links.
7. Keep the document visually coherent. Use a stable design language with a
   clear header, current state, details, open questions, evidence, links,
   tables, callouts, diagrams, and collapsible detail where appropriate.
8. Always update the HTML title element. The title element is the canonical
   session title shown in the browser sidebar. Keep the visible main heading
   synchronized with it.
9. When referring to a local source file, include a VS Code link whenever a
   location is known:
   vscode://file/absolute/path/to/file:line:column
   Encode path characters as required by a URL. Line and column are 1-based.
10. After a successful edit, keep the chat response minimal, for example
    "更新しました。続行します。"

If the artifact command or file edit is genuinely unavailable after reasonable
attempts, answer normally in chat and explain that the artifact update failed.
Do not silently lose a substantive answer.

Provider for this skill: {provider_name}.
"#
    )
}

pub fn hook_context(provider: Provider, input: &str) -> String {
    let parsed: Value = serde_json::from_str(input).unwrap_or_else(|_| json!({}));
    let session_id = parsed
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let cwd = parsed
        .get("cwd")
        .or_else(|| parsed.get("working_directory"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let event = parsed
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("session");
    let command = format!(
        "session-artifacts open --provider {} --session-id {} --cwd {} --json",
        provider.as_str(),
        shell_quote(&session_id),
        shell_quote(&cwd)
    );
    let context = format!(
        "Session artifact context ({event}).\n\
Provider: {}\n\
Session ID: {}\n\
Session cwd: {}\n\
Obtain the artifact path by running this command through the execution mechanism available in the current environment:\n\
{}\n\
The hook only provides this information; it does not execute the command. After it succeeds, edit the returned artifact_path with normal file tools. Keep substantive responses in the HTML and update its <title>.",
        provider.as_str(),
        session_id,
        cwd,
        command
    );

    match provider {
        Provider::Claude | Provider::Codex => {
            let hook_event = parsed
                .get("hook_event_name")
                .and_then(Value::as_str)
                .unwrap_or("SessionStart");
            serde_json::to_string(&json!({
                "hookSpecificOutput": {
                    "hookEventName": hook_event,
                    "additionalContext": context
                }
            }))
            .unwrap_or(context)
        }
        Provider::Generic => context,
    }
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
        "Only session-artifacts hook entries were removed; unrelated provider settings were preserved."
            .to_string(),
    );
    Ok(result)
}

fn install_claude(result: &mut InstallResult) -> Result<()> {
    let home = home_dir()?;
    let skill_dir = home
        .join(".claude")
        .join("skills")
        .join("session-artifacts");
    write_if_changed(&skill_dir.join("SKILL.md"), &skill_text(Provider::Claude))?;
    result
        .installed
        .push(skill_dir.join("SKILL.md").display().to_string());

    let executable = env::current_exe()?;
    let command = format!(
        "{} hook --provider claude",
        shell_quote(&executable.to_string_lossy())
    );
    let settings_path = home.join(".claude").join("settings.json");
    let changed = add_claude_hooks(&settings_path, &command)?;
    if changed {
        result.installed.push(settings_path.display().to_string());
    } else {
        result.skipped.push(format!(
            "{} already contains the session-artifacts hooks",
            settings_path.display()
        ));
    }
    Ok(())
}

fn uninstall_claude(result: &mut UninstallResult) -> Result<()> {
    let home = home_dir()?;
    let skill_path = home
        .join(".claude")
        .join("skills")
        .join("session-artifacts")
        .join("SKILL.md");
    remove_skill_file(&skill_path, result)?;

    let settings_path = home.join(".claude").join("settings.json");
    if remove_provider_hooks(&settings_path, &Provider::Claude)? {
        result.removed.push(settings_path.display().to_string());
    }
    Ok(())
}

fn codex_hook_context_text() -> String {
    format!(
        "# Codex hook context\n\n\
Codex should provide the current session_id and cwd to an integration hook.\n\
The hook must tell the agent to run the following operation, without running it\n\
itself:\n\n\
    session-artifacts hook --provider codex\n\n\
The command reads the provider event JSON from stdin and emits the context that\n\
should be injected into the agent. The hook must not create the artifact or\n\
assume that the local executable is available in a remote environment.\n\n\
Generated by session-artifacts {}.\n",
        env!("CARGO_PKG_VERSION")
    )
}

fn install_codex(result: &mut InstallResult) -> Result<()> {
    let home = home_dir()?;
    let skill_dir = home.join(".codex").join("skills").join("session-artifacts");
    write_if_changed(&skill_dir.join("SKILL.md"), &skill_text(Provider::Codex))?;
    result
        .installed
        .push(skill_dir.join("SKILL.md").display().to_string());

    let hook_template = skill_dir.join("HOOK-CONTEXT.md");
    let content = codex_hook_context_text();
    write_if_changed(&hook_template, &content)?;
    result.installed.push(hook_template.display().to_string());

    let executable = env::current_exe()?;
    let command = format!(
        "{} hook --provider codex",
        shell_quote(&executable.to_string_lossy())
    );
    let hooks_path = home.join(".codex").join("hooks.json");
    if add_codex_hooks(&hooks_path, &command)? {
        result.installed.push(hooks_path.display().to_string());
    } else {
        result.skipped.push(format!(
            "{} already contains the session-artifacts hooks",
            hooks_path.display()
        ));
    }

    let config_path = home.join(".codex").join("config.toml");
    if enable_codex_hooks(&config_path)? {
        result.installed.push(config_path.display().to_string());
    } else {
        result.skipped.push(format!(
            "{} already enables features.codex_hooks",
            config_path.display()
        ));
    }
    result.notes.push(
        "Codex SessionStart and UserPromptSubmit hooks inject session context only; they do not create or edit the artifact."
            .to_string(),
    );
    Ok(())
}

fn uninstall_codex(result: &mut UninstallResult) -> Result<()> {
    let home = home_dir()?;
    let skill_dir = home.join(".codex").join("skills").join("session-artifacts");
    remove_skill_file(&skill_dir.join("SKILL.md"), result)?;
    remove_generated_file(
        &skill_dir.join("HOOK-CONTEXT.md"),
        &codex_hook_context_text(),
        result,
    )?;

    let hooks_path = home.join(".codex").join("hooks.json");
    if remove_provider_hooks(&hooks_path, &Provider::Codex)? {
        result.removed.push(hooks_path.display().to_string());
    }
    result.notes.push(
        "features.codex_hooks was left unchanged so other Codex hooks are not disabled."
            .to_string(),
    );
    Ok(())
}

fn add_claude_hooks(path: &Path, command: &str) -> Result<bool> {
    let mut settings: Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(path)?)?
    } else {
        json!({})
    };
    let hooks = settings
        .as_object_mut()
        .ok_or("Claude settings root must be a JSON object")?
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or("Claude settings hooks must be a JSON object")?;
    let mut changed = false;
    for event in ["SessionStart", "UserPromptSubmit"] {
        let handlers = hooks.entry(event).or_insert_with(|| json!([]));
        let handlers = handlers
            .as_array_mut()
            .ok_or("Claude hook event must be an array")?;
        let entry = json!({
            "hooks": [
                {
                    "type": "command",
                    "command": command
                }
            ]
        });
        if !handlers.iter().any(|existing| existing == &entry) {
            handlers.push(entry);
            changed = true;
        }
    }
    if changed {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_if_changed(path, &serde_json::to_string_pretty(&settings)?)?;
    }
    Ok(changed)
}

fn add_codex_hooks(path: &Path, command: &str) -> Result<bool> {
    let mut settings: Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(path)?)?
    } else {
        json!({})
    };
    let hooks = settings
        .as_object_mut()
        .ok_or("Codex hooks root must be a JSON object")?
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or("Codex hooks must be a JSON object")?;
    let mut changed = false;
    for event in ["SessionStart", "UserPromptSubmit"] {
        let handlers = hooks.entry(event).or_insert_with(|| json!([]));
        let handlers = handlers
            .as_array_mut()
            .ok_or("Codex hook event must be an array")?;
        let entry = json!({
            "hooks": [
                {
                    "type": "command",
                    "command": command
                }
            ]
        });
        if !handlers.iter().any(|existing| existing == &entry) {
            handlers.push(entry);
            changed = true;
        }
    }
    if changed {
        write_if_changed(path, &serde_json::to_string_pretty(&settings)?)?;
    }
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

fn remove_generated_file(
    path: &Path,
    expected_content: &str,
    result: &mut UninstallResult,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if fs::read_to_string(path)? != expected_content {
        result
            .skipped
            .push(format!("{} was modified; left it in place", path.display()));
        return Ok(());
    }
    fs::remove_file(path)?;
    result.removed.push(path.display().to_string());
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
    Ok(())
}

fn remove_provider_hooks(path: &Path, provider: &Provider) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let mut settings: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
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
    let mut changed = false;
    for event_name in event_names {
        let Some(groups_value) = hooks.get_mut(&event_name) else {
            continue;
        };
        let groups = groups_value
            .as_array_mut()
            .ok_or("hook event must be an array")?;
        let mut remaining_groups = Vec::with_capacity(groups.len());
        for mut group in groups.drain(..) {
            let remove_group = if let Some(group_object) = group.as_object_mut() {
                if let Some(group_hooks_value) = group_object.get_mut("hooks") {
                    let group_hooks = group_hooks_value
                        .as_array_mut()
                        .ok_or("hook group hooks must be an array")?;
                    let original_len = group_hooks.len();
                    group_hooks.retain(|hook| !is_session_artifacts_hook(hook, provider));
                    if group_hooks.len() != original_len {
                        changed = true;
                    }
                    group_hooks.is_empty()
                } else {
                    false
                }
            } else {
                false
            };
            if remove_group {
                changed = true;
            } else {
                remaining_groups.push(group);
            }
        }
        *groups = remaining_groups;
        if groups.is_empty() {
            hooks.remove(&event_name);
            changed = true;
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
        changed = true;
    }
    if changed {
        write_if_changed(path, &serde_json::to_string_pretty(&settings)?)?;
    }
    Ok(changed)
}

fn is_session_artifacts_hook(value: &Value, provider: &Provider) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("type").and_then(Value::as_str) != Some("command") {
        return false;
    }
    let Some(command) = object.get("command").and_then(Value::as_str) else {
        return false;
    };
    let suffix = format!(" hook --provider {}", provider.as_str());
    command.ends_with(&suffix)
        && (command.contains("session-artifacts") || command.contains("session_artifacts"))
}

fn enable_codex_hooks(path: &Path) -> Result<bool> {
    let before = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    let after = ensure_codex_hooks_feature(&before)?;
    if after == before {
        return Ok(false);
    }
    write_if_changed(path, &after)?;
    Ok(true)
}

fn ensure_codex_hooks_feature(document: &str) -> Result<String> {
    let had_trailing_newline = document.ends_with('\n');
    let mut lines: Vec<String> = document.lines().map(ToOwned::to_owned).collect();

    if let Some(section_start) = lines.iter().position(|line| line.trim() == "[features]") {
        let section_end = lines
            .iter()
            .enumerate()
            .skip(section_start + 1)
            .find(|(_, line)| line.trim_start().starts_with('['))
            .map(|(index, _)| index)
            .unwrap_or(lines.len());
        if let Some(feature_line) = (section_start + 1..section_end)
            .find(|&index| is_toml_key_line(&lines[index], "codex_hooks"))
        {
            if lines[feature_line].trim() == "codex_hooks = true" {
                return Ok(document.to_string());
            }
            lines[feature_line] = "codex_hooks = true".to_string();
        } else {
            lines.insert(section_start + 1, "codex_hooks = true".to_string());
        }
    } else {
        let root_has_inline_features = lines
            .iter()
            .take_while(|line| !line.trim_start().starts_with('['))
            .any(|line| is_toml_key_line(line, "features"));
        if root_has_inline_features {
            return Err(
                "Codex config uses an inline `features` table; update it to include codex_hooks = true"
                    .into(),
            );
        }
        if !lines.is_empty() && !lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[features]".to_string());
        lines.push("codex_hooks = true".to_string());
    }

    let mut result = lines.join("\n");
    if had_trailing_newline || !result.is_empty() {
        result.push('\n');
    }
    Ok(result)
}

fn is_toml_key_line(line: &str, key: &str) -> bool {
    let line = line.trim_start();
    let Some(remainder) = line.strip_prefix(key) else {
        return false;
    };
    remainder
        .chars()
        .next()
        .is_some_and(|character| character.is_whitespace() || character == '=')
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

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_hook_context_is_json() {
        let output = hook_context(
            Provider::Codex,
            r#"{"session_id":"session-123","cwd":"/tmp/project","hook_event_name":"SessionStart"}"#,
        );
        let output: Value = serde_json::from_str(&output).expect("valid Codex hook output");
        assert_eq!(
            output["hookSpecificOutput"]["hookEventName"],
            "SessionStart"
        );
        assert!(
            output["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .expect("context")
                .contains("session-123")
        );
    }

    #[test]
    fn skill_has_yaml_frontmatter() {
        let skill = skill_text(Provider::Codex);
        assert!(skill.starts_with("---\n"));
        assert!(skill.contains("name: session-artifacts\n"));
        assert!(skill.contains("description: Maintain one live structured HTML artifact"));
    }

    #[test]
    fn codex_feature_is_added_without_rewriting_other_config() {
        let input =
            "model = \"gpt-5\"\n\n[projects]\n\"/tmp/project\" = { trust_level = \"trusted\" }\n";
        let output = ensure_codex_hooks_feature(input).expect("feature update");
        assert!(output.contains("model = \"gpt-5\""));
        assert!(output.contains("[projects]"));
        assert!(output.contains("[features]\ncodex_hooks = true"));
    }

    #[test]
    fn existing_codex_feature_is_enabled() {
        let input = "[features]\ncodex_hooks = false\n\n[projects]\n";
        let output = ensure_codex_hooks_feature(input).expect("feature update");
        assert_eq!(output, "[features]\ncodex_hooks = true\n\n[projects]\n");
    }

    #[test]
    fn codex_hooks_are_registered_idempotently() {
        let path = std::env::temp_dir().join(format!(
            "session-artifacts-hooks-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        assert!(
            add_codex_hooks(&path, "/tmp/session-artifacts hook --provider codex")
                .expect("write hooks")
        );
        assert!(
            !add_codex_hooks(&path, "/tmp/session-artifacts hook --provider codex")
                .expect("second hooks write")
        );
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).expect("read hooks"))
            .expect("valid hooks JSON");
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["type"],
            "command"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn uninstall_removes_a_modified_skill_file() {
        let directory = std::env::temp_dir().join(format!(
            "session-artifacts-skill-uninstall-test-{}",
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
