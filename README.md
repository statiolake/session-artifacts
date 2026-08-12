# session-whiteboard

Coding-agent sessions have one live, structured HTML whiteboard each. The agent
re-renders that whiteboard with its ordinary file tools; a local daemon serves
it to a browser and reloads the view as the file changes.

The whiteboard is deliberately not a transcript or archive. It is a one-screen
projection of the current focus, with only the context anchors needed to keep
the larger picture visible. Its `<title>` is also the canonical title shown in
the browser sidebar.

## Install

Rust is the only build dependency:

    cargo install --path .
    session-whiteboard install

`install` is a global, one-time provider integration. It installs the skill and
shell-only hook instructions for Claude Code and Codex. The hooks use ubiquitous
shell tools such as `echo`, pass the provider's session ID and cwd to the agent,
and tell the agent how to obtain the whiteboard path. They never invoke the
`session-whiteboard` binary directly, so an agent running inside a container can
choose the host, proxy, MCP, or other execution mechanism available there.

The hooks are registered for `SessionStart`, `UserPromptSubmit`, and `Stop`.
The `Stop` hook adds one explicit turn-end reminder to replace the existing HTML
with the current board; it allows the turn to finish after that reminder has
been delivered once. Reinstalling migrates the old `session-artifacts` hooks and
skill files to the new name.

Install one provider explicitly when needed:

    session-whiteboard install --provider claude
    session-whiteboard install --provider codex

Remove the global integrations when they are no longer wanted:

    session-whiteboard uninstall

Uninstall removes the session-whiteboard skill files and only the matching
session-whiteboard hook entries. It preserves unrelated provider settings. The
Codex hook feature flag is retained so other Codex hooks are not disabled.

## Session workflow

The agent runs this command when a session starts or when the whiteboard is needed:

    session-whiteboard open \
      --provider codex \
      --session-id <id> \
      --cwd "$PWD" \
      --json

The JSON response contains `artifact_path`, relative to `relative_to`, and a
`viewer_url`. The first open for a daemon process automatically opens that URL
in the system browser. The URL is the navigation shell: active sessions are on
the left and the selected whiteboard is on the right. The daemon creates the
file at:

    <session-cwd>/.session-whiteboard/<provider>/<session-key>.html

If the cwd is inside a Git repository, the first `open` adds the exact
`.session-whiteboard/` directory to that repository's `.git/info/exclude`.
Failure to update the exclude file is reported as a warning and does not block
artifact creation.

When a provider session ends, mark it inactive while retaining its HTML for a
future resume:

    session-whiteboard close --provider codex --session-id <id> --cwd "$PWD"

The browser hides inactive sessions. Re-opening the same provider/session/cwd
reactivates the same file. Deletion is intentionally not automatic in this MVP.

To explicitly delete the HTML and its registry record:

    session-whiteboard delete --provider codex --session-id <id> --cwd "$PWD"

## Browser viewer

The first `open` automatically starts the daemon and opens the navigation shell.
The sidebar groups active sessions by their session-start cwd and the main pane
live-reloads the selected HTML whiteboard. Later opens for the same daemon do
not open duplicate browser windows.

## Development

    cargo run -- open --provider codex --session-id demo --cwd "$PWD" --json
    cargo run -- skill --provider codex
    cargo test
    cargo clippy --all-targets --all-features -- -D warnings

The current release targets local macOS/Linux-style workflows. VS Code links
(`vscode://file/...:line:column`) are supported in the agent instructions;
remote workspace URI mapping and editor extensions are outside this MVP.
