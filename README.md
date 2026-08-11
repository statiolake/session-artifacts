# session-artifacts

Coding-agent sessions have one live, structured HTML document each. The agent
edits that document with its ordinary file tools; a local daemon serves it to a
browser and reloads the view as the file changes.

The artifact is deliberately not a transcript. It is the current structured
understanding of the session: conclusions, open questions, evidence, links,
code references, and next actions. Its `<title>` is also the canonical title
shown in the browser sidebar.

## Install

Rust is the only build dependency:

    cargo install --path .
    session-artifacts install

`install` is a global, one-time provider integration. It installs the skill and
hook context for Claude Code and Codex. The hooks only inject the provider's
session ID, cwd, and the command for obtaining the artifact path; they do not
create or edit the artifact themselves. The command can then be run through
whatever local, remote, or MCP-backed execution mechanism is available.

Install one provider explicitly when needed:

    session-artifacts install --provider claude
    session-artifacts install --provider codex

Remove the global integrations when they are no longer wanted:

    session-artifacts uninstall

Uninstall removes the session-artifacts skill files and only the matching
session-artifacts hook entries. It preserves unrelated provider settings. The
Codex hook feature flag is retained so other Codex hooks are not disabled.

## Session workflow

The agent runs this command when a session starts or when the artifact is needed:

    session-artifacts open \
      --provider codex \
      --session-id <id> \
      --cwd "$PWD" \
      --json

The JSON response contains `artifact_path`, relative to `relative_to`, and a
`viewer_url`. The daemon creates the file at:

    <session-cwd>/.session-artifacts/<provider>/<session-key>.html

If the cwd is inside a Git repository, the first `open` adds the exact
`.session-artifacts/` directory to that repository's `.git/info/exclude`.
Failure to update the exclude file is reported as a warning and does not block
artifact creation.

When a provider session ends, mark it inactive while retaining its HTML for a
future resume:

    session-artifacts close --provider codex --session-id <id> --cwd "$PWD"

The browser hides inactive sessions. Re-opening the same provider/session/cwd
reactivates the same file. Deletion is intentionally not automatic in this MVP.

To explicitly delete the HTML and its registry record:

    session-artifacts delete --provider codex --session-id <id> --cwd "$PWD"

## Browser viewer

The first `open` automatically starts the daemon. Open the returned
`viewer_url`; the sidebar groups active sessions by their session-start cwd and
the main pane live-reloads the selected HTML document.

## Development

    cargo run -- open --provider codex --session-id demo --cwd "$PWD" --json
    cargo run -- skill --provider codex
    cargo test
    cargo clippy --all-targets --all-features -- -D warnings

The current release targets local macOS/Linux-style workflows. VS Code links
(`vscode://file/...:line:column`) are supported in the agent instructions;
remote workspace URI mapping and editor extensions are outside this MVP.
