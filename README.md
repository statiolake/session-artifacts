# session-whiteboard

Coding-agent sessions have one live, structured HTML whiteboard each. The agent
re-renders that whiteboard with its ordinary file tools; a local daemon serves
it to a browser and reloads the view as the file changes.

The whiteboard is deliberately not a transcript or archive. It is a compact
two-dimensional explanation for the engineer reading it: grouping, position,
and relationships carry meaning that a linear chat stream loses. Keep only the
necessary entities, evidence, decisions, and next action visible. Its `<title>`
is also the canonical title shown in the browser sidebar.

## Install

Rust is the only build dependency:

    cargo install --path .
    session-whiteboard install

`install` is a global, one-time provider integration. It installs the skill and
shell-only hook instructions for Claude Code and Codex. The hooks use ubiquitous
shell tools, pass the provider's session ID and cwd to the agent, and emit
Codex's required structured JSON for context events,
and tell the agent how to obtain the whiteboard path. Context hooks never invoke
the `session-whiteboard` binary directly, so an agent running inside a container
can choose the host, proxy, MCP, or other execution mechanism available there.

The hooks are registered for `SessionStart`, `UserPromptSubmit`, and `Stop`.
They only inject session metadata and instructions; they do not execute the
binary or edit the board. The `Stop` hook adds one explicit turn-end reminder
to replace the existing HTML with the current board. There is no `SessionEnd`
cleanup hook: cleanup is explicit because that lifecycle event cannot ask the
agent to rewrite the board. Reinstalling migrates old `session-artifacts`
hooks and skill files, including removing the old `SessionEnd` entry, to the
new name.

Install one provider explicitly when needed:

    session-whiteboard install --provider claude
    session-whiteboard install --provider codex

Remove the global integrations when they are no longer wanted:

    session-whiteboard uninstall

Uninstall removes the session-whiteboard skill files and only the matching
session-whiteboard hook entries. It preserves unrelated provider settings. The
Codex hook feature flags are retained so other Codex hooks are not disabled.

## Design language

The included `.interface-design/system.md` records the whiteboard's visual
direction: a bright paper surface, graphite notation, one blue marker accent,
a broad explanation field with narrow marginalia, and dense but readable
spatial structure. It does not prescribe a giant hero heading or a fixed
content checklist. Secondary overflow may be exposed through an explicit
popover or details control; essential overflow may scroll rather than overlap
or clip. Local source references are copy-to-clipboard actions showing a
portable relative `path:line`, rather than an editor-specific URL.

## Session workflow

The agent runs this command when a session starts or when the whiteboard is needed:

    session-whiteboard prepare \
      --provider codex \
      --session-id <id> \
      --cwd "$PWD" \
      --json

The JSON response contains `artifact_path`, relative to `relative_to`, and a
session-specific `viewer_url`. `prepare` opens the navigation shell only when
the viewer has not been seen recently or its keep-alive connection is gone. It
never changes the browser's current whiteboard selection. Active sessions are
on the left and the selected whiteboard is on the right. The daemon creates the
file at:

    <session-cwd>/.session-whiteboard/<provider>/<session-key>.html

If the cwd is inside a Git repository, the first `prepare` adds the exact
`.session-whiteboard/` directory to that repository's `.git/info/exclude`.
Failure to update the exclude file is reported as a warning and does not block
artifact creation.

To open only the navigation viewer, without registering or selecting a session:

    session-whiteboard browse

`browse` starts the managed daemon if needed and invokes the browser launcher
every time. It does not automatically switch the selected session. Choose
sessions from the sidebar. Since a generic OS browser launcher cannot reliably
reuse an arbitrary existing tab, `browse` may open another tab; use `prepare`
for the non-duplicating keep-alive-aware behavior.

When a provider session ends, mark it inactive while retaining its HTML for a
future resume:

    session-whiteboard close --provider codex --session-id <id> --cwd "$PWD"

The browser hides inactive sessions. Re-opening the same provider/session/cwd
reactivates the same file. The sidebar also has a per-session inactive action
for recovering from a missed provider exit hook. Deletion is intentionally not
automatic in this MVP.

To explicitly clean (delete) the HTML and its registry record:

    session-whiteboard clean --provider codex --session-id <id> --cwd "$PWD"

## Browser viewer

The first `prepare` automatically starts the daemon and opens the navigation shell
when no viewer connection is present.
The sidebar groups active sessions by their session-start cwd and the main pane
live-reloads the selected HTML whiteboard. Deactivated sessions remain available
under a collapsed inactive toggle, and active sessions are ordered by the
whiteboard file's latest modification time. Artifact changes are detected by the
daemon's filesystem watcher and pushed to the viewer for immediate reload. A
changed session that is not currently selected receives an unread marker until
it is opened. Session preparation refreshes the sidebar but does not
automatically switch the current selection. `prepare` reopens the viewer when
its event connection is gone; `browse` deliberately invokes the browser
launcher even when a viewer is already open. Browser/OS launchers cannot force
reuse of an arbitrary already-open tab across all browser families, so
keep-alive-aware preparation is the single-tab-friendly path.

Manage the background daemon explicitly when needed:

    session-whiteboard daemon start
    session-whiteboard daemon stop
    session-whiteboard daemon restart

## Development

    cargo run -- prepare --provider codex --session-id demo --cwd "$PWD" --json
    cargo run -- browse --json
    cargo run -- skill --provider codex
    cargo test
    cargo clippy --all-targets --all-features -- -D warnings

The current release targets local macOS/Linux-style workflows. Editor-specific
URI schemes and remote workspace mapping are outside this MVP; source references
stay portable as copied `relative/path:line` text.
