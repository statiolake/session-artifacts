use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::model::{OpenRequest, Provider};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const INSTALLED_HOOK_EVENTS: [&str; 4] = ["SessionStart", "UserPromptSubmit", "Stop", "SessionEnd"];
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
description: Maintain one live structured HTML whiteboard as the current context view of an agent session.
---

# Session Whiteboard

Use the session whiteboard as the primary user-facing view for this session.
It is a disposable, current-context projection, not a transcript, archive, or
complete knowledge base. The chat response is only a short transport/status
message after a successful update.

## Required workflow

1. Read the session-whiteboard reminder injected by the provider hook. It contains
   the provider, the current session ID, and the session cwd.
2. Obtain the artifact path through the execution mechanism that works in the
   current environment. The hook only gives instructions; it does not execute
   `session-whiteboard` and it does not create the file. If a direct executable
   is unavailable, inspect the available environment and use the appropriate
   host, proxy, MCP, or other configured mechanism before giving up.
3. Use the returned artifact_path as a path relative to relative_to. Do not
   create a replacement file in the repository and do not choose another path.
4. Re-render the complete HTML document from the current turn and replace the
   existing file. Do not append a new log entry or preserve stale content just
   for completeness. The previous whiteboard is a disposable draft.
5. Keep the current focus dominant. Retain only the compact anchors needed to
   understand it, plus current decisions, one unresolved point, and the next
   useful action. Delete old branches, solved questions, and irrelevant detail.
6. Do not follow a fixed content template. Use a stable visual language instead:
   spatial grouping, clear hierarchy, compact labels, and relationships that can
   be understood at a glance. Prefer a compact single viewport, but allow natural
   scrolling whenever the alternative would clip or overlap essential content.
7. Keep the HTML self-contained. Inline CSS, JavaScript, SVG, and small data
   are preferred. External links are allowed when useful, including local file
   links.
8. Always update the HTML title element. The title element is the canonical
   session title shown in the browser sidebar. Keep the visible main heading
   synchronized with it.
9. When referring to a local source file, include the visible relative/path:line
   as a copy action that writes that exact string to the clipboard. Do not use
   an editor-specific URL scheme. Keep the readable path and line visible if
   the Clipboard API is not available.
10. Keep the normal board compact. If secondary context does not fit, collapse it
   behind an explicit click target such as <details> or a dialog/popover and label
   what is hidden. If essential context still needs more room, let the document
   scroll; never allow cards to overlap or become unreadable, and do not turn the
   board into a long scrolling transcript.
11. Read .interface-design/system.md when it is present and follow its paper
   whiteboard contract: 4px spacing rhythm, warm paper surface, graphite rules,
   one blue marker accent, a broad focus field with narrow marginalia, and no
   generic equal-card grid. The current focus must visually lead.
12. SessionEnd is a cleanup hook rather than a context reminder: it deactivates
    this session in the registry so the viewer can keep it available under the
    collapsed inactive list.
13. After a successful edit, keep the chat response minimal, for example
    "更新しました。続行します。"

If the whiteboard command or file edit is genuinely unavailable after
reasonable attempts, answer normally in chat and explain that the whiteboard
update failed. Do not silently lose a substantive answer.

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
        "session-whiteboard open --provider {} --session-id {} --cwd {} --json",
        provider.as_str(),
        shell_quote(&session_id),
        shell_quote(&cwd)
    );
    let context = format!(
        "Session whiteboard context ({event}).\n\
Provider: {}\n\
Session ID: {}\n\
Session cwd: {}\n\
Obtain the artifact path by running this command through the execution mechanism available in the current environment:\n\
{}\n\
The hook only provides this information; it does not execute the command. After it succeeds, replace the returned artifact_path with the complete current whiteboard. Keep substantive responses in the HTML and update its <title>.",
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

pub fn hook_session_end_request(provider: Provider, input: &str) -> Result<OpenRequest> {
    let parsed: Value = serde_json::from_str(input)?;
    let session_id = parsed
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("SessionEnd hook input has no session_id")?;
    let cwd = parsed
        .get("cwd")
        .or_else(|| parsed.get("working_directory"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("SessionEnd hook input has no cwd")?;
    Ok(OpenRequest {
        provider,
        session_id: session_id.to_string(),
        cwd: cwd.into(),
    })
}

fn hook_context_command(provider: &Provider, event: &str) -> String {
    let marker = format!(
        "[{HOOK_MARKER} provider={} event={event}]",
        provider.as_str()
    );
    let instruction = format!(
        "{marker}\n\
Session-whiteboard integration for {event}.\n\
This hook deliberately uses only the environment's shell tools; it does not run\n\
the session-whiteboard binary. The session_id and cwd below are from the provider\n\
hook payload. Use them to obtain the artifact path through whatever mechanism is\n\
available in this environment:\n\
session-whiteboard open --provider {} --session-id <session_id> --cwd <cwd> --json\n\
Then replace the returned artifact_path with the complete current whiteboard.\n\
Keep substantive answers in the HTML and update its <title>.",
        provider.as_str()
    );
    match provider {
        Provider::Codex => codex_hook_context_command(provider, event, &marker),
        Provider::Claude | Provider::Generic => {
            let instruction = shell_quote(&instruction);
            format!(
                "{HOOK_INPUT_PARSER}; printf '%s\\n' {instruction}; printf 'session_id=%s\\ncwd=%s\\n' \"$session_id\" \"$cwd\""
            )
        }
    }
}

const HOOK_INPUT_PARSER: &str = r#"input=$(cat); session_id=$(printf '%s' "$input" | sed -n 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'); cwd=$(printf '%s' "$input" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'); if [ -z "$cwd" ]; then cwd=$(printf '%s' "$input" | sed -n 's/.*"working_directory"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'); fi"#;

fn codex_hook_context_command(provider: &Provider, event: &str, marker: &str) -> String {
    let prefix = format!(
        "{marker}\n\
Session-whiteboard integration for {event}.\n\
This hook deliberately uses only the environment's shell tools; it does not run\n\
the session-whiteboard binary.\n\
Provider: {}\n\
Session ID: ",
        provider.as_str()
    );
    let middle = "\nSession cwd: ";
    let suffix = format!(
        "\nUse these values to obtain the artifact path through whatever mechanism is\n\
available in this environment:\n\
session-whiteboard open --provider {} --session-id <session_id> --cwd <cwd> --json\n\
Then replace the returned artifact_path with the complete current whiteboard.\n\
Keep substantive answers in the HTML and update its <title>.",
        provider.as_str()
    );
    let output_start = format!(
        r#"{{"hookSpecificOutput":{{"hookEventName":{},"additionalContext":"{}"#,
        serde_json::to_string(event).expect("Codex hook event is serializable"),
        json_string_fragment(&prefix)
    );
    let output_end = format!("{}\"}}}}", json_string_fragment(&suffix));
    let output_start = shell_quote(&output_start);
    let output_middle = shell_quote(&json_string_fragment(middle));
    let output_end = shell_quote(&output_end);

    format!(
        "{HOOK_INPUT_PARSER}; session_id_json=$(printf '%s' \"$session_id\" | sed -e 's/\\\\/\\\\\\\\/g' -e 's/\"/\\\\\"/g'); cwd_json=$(printf '%s' \"$cwd\" | sed -e 's/\\\\/\\\\\\\\/g' -e 's/\"/\\\\\"/g'); printf '%s%s%s%s%s\\n' {output_start} \"$session_id_json\" {output_middle} \"$cwd_json\" {output_end}"
    )
}

fn json_string_fragment(value: &str) -> String {
    let encoded = serde_json::to_string(value).expect("JSON string is serializable");
    encoded[1..encoded.len() - 1].to_string()
}

fn stop_hook_command(provider: &Provider) -> String {
    let marker = format!("[{HOOK_MARKER} provider={} event=Stop]", provider.as_str());
    let reason = format!(
        "{marker} Turn-end instruction: check the session whiteboard before this turn finishes. Did you re-render it during this turn? If not, rebuild the complete HTML now around the current focus. Keep only essential context anchors and the next useful action; delete stale branches. Do not only describe the update in chat. If the session-whiteboard command is unavailable here, find the environment-appropriate way to run `session-whiteboard open --provider {}` using the session_id and cwd from the hook context, then replace the returned artifact_path. After checking or updating the whiteboard, you may finish the turn.",
        provider.as_str()
    );
    let output = serde_json::to_string(&json!({
        "decision": "block",
        "reason": reason,
    }))
    .expect("stop hook output is serializable");

    format!(
        "input=$(cat); stop_hook_active=$(printf '%s' \"$input\" | sed -n 's/.*\"stop_hook_active\"[[:space:]]*:[[:space:]]*true.*/true/p'); if [ \"$stop_hook_active\" = true ]; then echo '{{}}'; else echo {}; fi",
        shell_quote(&output)
    )
}

fn hook_command(provider: &Provider, event: &str) -> String {
    if event == "Stop" {
        stop_hook_command(provider)
    } else if event == "SessionEnd" {
        session_end_hook_command(provider)
    } else {
        hook_context_command(provider, event)
    }
}

fn session_end_hook_command(provider: &Provider) -> String {
    let executable = env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| PRODUCT_NAME.to_string());
    format!(
        "{} session-end --provider {} # [{HOOK_MARKER} provider={} event=SessionEnd]",
        shell_quote(&executable),
        provider.as_str(),
        provider.as_str()
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
    let changed = add_claude_hooks(&settings_path, &Provider::Claude)?;
    if changed {
        result.installed.push(settings_path.display().to_string());
    } else {
        result.skipped.push(format!(
            "{} already contains the session-whiteboard hooks",
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

fn codex_hook_context_text() -> String {
    codex_hook_context_text_for(PRODUCT_NAME)
}

fn codex_hook_context_text_for(product: &str) -> String {
    format!(
        "# Codex hook context\n\n\
Codex SessionStart and UserPromptSubmit hooks emit the provider's structured\n\
hookSpecificOutput JSON. They are intentionally shell-only and pass the\n\
session_id and cwd from the provider payload to the agent; they do not execute\n\
the {product} binary. The agent should run the following operation\n\
through the mechanism available in its current environment:\n\n\
    {product} open --provider codex --session-id <session_id> --cwd <cwd> --json\n\n\
The Stop hook adds one explicit reminder to update the existing artifact before\n\
the turn finishes. It guards the continuation with stop_hook_active so it does\n\
not create an unbounded loop. SessionEnd invokes the installed executable to\n\
deactivate the session in the daemon registry.\n\n\
Generated by {product} {}.\n",
        env!("CARGO_PKG_VERSION")
    )
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
    let content = codex_hook_context_text();
    write_if_changed(&hook_template, &content)?;
    result.installed.push(hook_template.display().to_string());

    let hooks_path = home.join(".codex").join("hooks.json");
    if add_codex_hooks(&hooks_path, &Provider::Codex)? {
        result.installed.push(hooks_path.display().to_string());
    } else {
        result.skipped.push(format!(
            "{} already contains the session-whiteboard hooks",
            hooks_path.display()
        ));
    }

    let config_path = home.join(".codex").join("config.toml");
    if enable_codex_hooks(&config_path)? {
        result.installed.push(config_path.display().to_string());
    } else {
        result.skipped.push(format!(
            "{} already enables Codex hooks",
            config_path.display()
        ));
    }
    result.notes.push("Codex context hooks emit valid hookSpecificOutput JSON and use shell-only instructions; they do not execute the session-whiteboard binary or edit the artifact themselves. Stop adds one turn-end update reminder.".to_string());
    Ok(())
}

fn uninstall_codex(result: &mut UninstallResult) -> Result<()> {
    let home = home_dir()?;
    for product in [PRODUCT_NAME, LEGACY_PRODUCT_NAME] {
        let skill_dir = home.join(".codex").join("skills").join(product);
        remove_skill_file(&skill_dir.join("SKILL.md"), result)?;
        remove_generated_file(
            &skill_dir.join("HOOK-CONTEXT.md"),
            &codex_hook_context_text_for(product),
            result,
        )?;
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

fn add_claude_hooks(path: &Path, provider: &Provider) -> Result<bool> {
    add_provider_hooks(path, provider)
}

fn add_codex_hooks(path: &Path, provider: &Provider) -> Result<bool> {
    add_provider_hooks(path, provider)
}

fn add_provider_hooks(path: &Path, provider: &Provider) -> Result<bool> {
    let mut settings: Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(path)?)?
    } else {
        json!({})
    };
    let before = settings.clone();
    let hooks = settings
        .as_object_mut()
        .ok_or("hook settings root must be a JSON object")?
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or("hook settings hooks must be a JSON object")?;

    let event_names: Vec<String> = hooks.keys().cloned().collect();
    for event_name in event_names {
        let canonical = INSTALLED_HOOK_EVENTS
            .iter()
            .find(|event| **event == event_name)
            .map(|event| hook_definition(provider, event));
        let Some(groups_value) = hooks.get_mut(&event_name) else {
            continue;
        };
        let groups = groups_value
            .as_array_mut()
            .ok_or("hook event must be an array")?;
        let event_changed = normalize_hook_groups(groups, provider, canonical.as_ref())?;
        if event_changed && groups.is_empty() {
            hooks.remove(&event_name);
        }
    }

    for event in INSTALLED_HOOK_EVENTS {
        if !hooks.contains_key(event) {
            hooks.insert(event.to_string(), json!([hook_entry(provider, event)]));
        }
    }

    let changed = settings != before;
    if changed {
        write_if_changed(path, &serde_json::to_string_pretty(&settings)?)?;
    }
    Ok(changed)
}

fn hook_entry(provider: &Provider, event: &str) -> Value {
    json!({
        "hooks": [hook_definition(provider, event)]
    })
}

fn hook_definition(provider: &Provider, event: &str) -> Value {
    let mut definition = json!({
        "type": "command",
        "command": hook_command(provider, event)
    });
    if event == "SessionEnd" {
        definition["timeout"] = json!(3);
    }
    definition
}

fn normalize_hook_groups(
    groups: &mut Vec<Value>,
    provider: &Provider,
    canonical: Option<&Value>,
) -> Result<bool> {
    let mut remaining_groups = Vec::with_capacity(groups.len());
    let mut found_canonical = false;
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
                        let is_canonical = !found_canonical
                            && canonical.is_some_and(|candidate| candidate == &hook);
                        if is_canonical {
                            found_canonical = true;
                            remaining_hooks.push(hook);
                        } else {
                            group_changed = true;
                            changed = true;
                        }
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

    if let Some(canonical) = canonical
        && !found_canonical
    {
        groups.push(json!({ "hooks": [canonical.clone()] }));
        changed = true;
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
        let event_changed = normalize_hook_groups(groups, provider, None)?;
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
        if let Some(feature_line) =
            (section_start + 1..section_end).find(|&index| is_toml_key_line(&lines[index], "hooks"))
        {
            if lines[feature_line].trim() == "hooks = true" {
                return Ok(document.to_string());
            }
            lines[feature_line] = "hooks = true".to_string();
        } else if let Some(legacy_feature_line) = (section_start + 1..section_end)
            .find(|&index| is_toml_key_line(&lines[index], "codex_hooks"))
        {
            if lines[legacy_feature_line].trim() == "codex_hooks = true" {
                return Ok(document.to_string());
            }
            lines[legacy_feature_line] = "hooks = true".to_string();
        } else {
            lines.insert(section_start + 1, "hooks = true".to_string());
        }
    } else {
        let root_has_inline_features = lines
            .iter()
            .take_while(|line| !line.trim_start().starts_with('['))
            .any(|line| is_toml_key_line(line, "features"));
        if root_has_inline_features {
            return Err(
                "Codex config uses an inline `features` table; update it to include hooks = true"
                    .into(),
            );
        }
        if !lines.is_empty() && !lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[features]".to_string());
        lines.push("hooks = true".to_string());
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
    fn session_end_hook_extracts_provider_identity() {
        let request = hook_session_end_request(
            Provider::Codex,
            r#"{"session_id":"session-123","cwd":"/tmp/project","hook_event_name":"SessionEnd"}"#,
        )
        .expect("valid SessionEnd input");
        assert_eq!(request.provider, Provider::Codex);
        assert_eq!(request.session_id, "session-123");
        assert_eq!(request.cwd, PathBuf::from("/tmp/project"));
    }

    #[test]
    fn skill_has_yaml_frontmatter() {
        let skill = skill_text(Provider::Codex);
        assert!(skill.starts_with("---\n"));
        assert!(skill.contains("name: session-whiteboard\n"));
        assert!(skill.contains("description: Maintain one live structured HTML whiteboard"));
    }

    #[test]
    fn codex_feature_is_added_without_rewriting_other_config() {
        let input =
            "model = \"gpt-5\"\n\n[projects]\n\"/tmp/project\" = { trust_level = \"trusted\" }\n";
        let output = ensure_codex_hooks_feature(input).expect("feature update");
        assert!(output.contains("model = \"gpt-5\""));
        assert!(output.contains("[projects]"));
        assert!(output.contains("[features]\nhooks = true"));
    }

    #[test]
    fn existing_legacy_codex_feature_is_preserved() {
        let input = "[features]\ncodex_hooks = true\n\n[projects]\n";
        let output = ensure_codex_hooks_feature(input).expect("feature update");
        assert_eq!(output, input);
    }

    #[test]
    fn disabled_legacy_codex_feature_is_migrated_to_current_key() {
        let input = "[features]\ncodex_hooks = false\n\n[projects]\n";
        let output = ensure_codex_hooks_feature(input).expect("feature update");
        assert_eq!(output, "[features]\nhooks = true\n\n[projects]\n");
    }

    #[test]
    fn disabled_current_codex_feature_is_enabled() {
        let input = "[features]\nhooks = false\n\n[projects]\n";
        let output = ensure_codex_hooks_feature(input).expect("feature update");
        assert_eq!(output, "[features]\nhooks = true\n\n[projects]\n");
    }

    #[test]
    fn codex_hooks_are_registered_idempotently() {
        let path = std::env::temp_dir().join(format!(
            "session-whiteboard-hooks-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        assert!(add_codex_hooks(&path, &Provider::Codex).expect("write hooks"));
        assert!(!add_codex_hooks(&path, &Provider::Codex).expect("second hooks write"));
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).expect("read hooks"))
            .expect("valid hooks JSON");
        for event in INSTALLED_HOOK_EVENTS {
            let command = settings["hooks"][event][0]["hooks"][0]["command"]
                .as_str()
                .expect("hook command");
            assert!(command.contains(HOOK_MARKER));
            assert!(!command.contains("/tmp/session-whiteboard"));
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn installing_hooks_migrates_legacy_binary_commands_and_preserves_other_hooks() {
        let path = std::env::temp_dir().join(format!(
            "session-whiteboard-hooks-migration-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let settings = json!({
            "hooks": {
                "SessionStart": [{
                    "hooks": [
                        {
                            "type": "command",
                            "command": "'/tmp/session-artifacts' hook --provider codex"
                        },
                        {
                            "type": "command",
                            "command": "other-hook"
                        }
                    ]
                }],
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": "'/tmp/session-artifacts' hook --provider codex"
                    }]
                }]
            }
        });
        fs::write(
            &path,
            serde_json::to_string_pretty(&settings).expect("serialize hooks"),
        )
        .expect("write hooks");

        assert!(add_codex_hooks(&path, &Provider::Codex).expect("migrate hooks"));
        let settings: Value = serde_json::from_str(&fs::read_to_string(&path).expect("read hooks"))
            .expect("valid hooks JSON");
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            "other-hook"
        );
        for event in INSTALLED_HOOK_EVENTS {
            let commands = settings["hooks"][event]
                .as_array()
                .expect("event groups")
                .iter()
                .flat_map(|group| group["hooks"].as_array().into_iter().flatten())
                .filter_map(|hook| hook["command"].as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                commands
                    .iter()
                    .filter(|command| command.contains(HOOK_MARKER))
                    .count(),
                1
            );
        }
        let _ = fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn shell_hooks_pass_only_session_metadata_and_stop_once() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        fn run(command: &str, input: &str) -> String {
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(command)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn shell hook");
            child
                .stdin
                .take()
                .expect("hook stdin")
                .write_all(input.as_bytes())
                .expect("write hook input");
            let output = child.wait_with_output().expect("read shell hook");
            assert!(output.status.success());
            String::from_utf8(output.stdout).expect("hook output is utf-8")
        }

        let output = run(
            &hook_command(&Provider::Codex, "SessionStart"),
            r#"{"session_id":"session-123","cwd":"/tmp/project","prompt":"ignore the artifact"}"#,
        );
        let output: Value = serde_json::from_str(&output).expect("valid Codex context output");
        assert_eq!(
            output["hookSpecificOutput"]["hookEventName"],
            "SessionStart"
        );
        let context = output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("Codex context");
        assert!(context.contains("Session ID: session-123"));
        assert!(context.contains("Session cwd: /tmp/project"));
        assert!(!context.contains("ignore the artifact"));
        assert!(context.contains("session-whiteboard open --provider codex"));

        let prompt_output = run(
            &hook_command(&Provider::Codex, "UserPromptSubmit"),
            r#"{"session_id":"session-123","working_directory":"/tmp/project"}"#,
        );
        let prompt_output: Value =
            serde_json::from_str(&prompt_output).expect("valid Codex prompt output");
        assert_eq!(
            prompt_output["hookSpecificOutput"]["hookEventName"],
            "UserPromptSubmit"
        );

        let first = run(
            &hook_command(&Provider::Codex, "Stop"),
            r#"{"stop_hook_active":false}"#,
        );
        let first: Value = serde_json::from_str(&first).expect("valid first stop output");
        assert_eq!(first["decision"], "block");
        assert!(
            first["reason"]
                .as_str()
                .unwrap()
                .contains("Did you re-render it")
        );

        let second = run(
            &hook_command(&Provider::Codex, "Stop"),
            r#"{"stop_hook_active":true}"#,
        );
        assert_eq!(second.trim(), "{}");
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
                                "command": hook_command(&Provider::Codex, "Stop")
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
