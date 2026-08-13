# session-whiteboard

Coding-agent sessions can have one live, structured HTML whiteboard. The agent
re-renders it with its ordinary file tools when the user asks for a whiteboard
or when a spatial explanation is clearly more useful; a local daemon serves one
board per URL and reloads the view as the file changes.

The whiteboard is deliberately not a transcript or archive. It is a compact
two-dimensional explanation for the engineer reading it: grouping, position,
and relationships carry meaning that a linear chat stream loses. Keep only the
necessary entities, evidence, decisions, and next action visible. Its `<title>`
names the board in the browser document.

## Install

Rust is the only build dependency:

    cargo install --path .
    session-whiteboard install

`install` is a global, one-time skill installation for Claude Code and Codex.
It does not install automatic hooks. If an older installation left
session-whiteboard hooks behind, reinstalling removes only those matching hook
entries and preserves unrelated provider settings. The skill is opt-in: it is
used for requests such as `ホワイトボードで説明して` or
`ちょっとそこホワイトボードにまとめて`, and may also be used when the
agent judges that spatial structure materially improves the explanation. It is
not required on every turn.

Install one provider explicitly when needed:

    session-whiteboard install --provider claude
    session-whiteboard install --provider codex

Remove the global integrations when they are no longer wanted:

    session-whiteboard uninstall

Uninstall removes the session-whiteboard skill files and only matching old
session-whiteboard hook entries. It preserves unrelated provider settings and
does not change Codex hook feature flags.

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

When the user requests a whiteboard, or when spatial structure materially
improves the explanation, the agent runs:

    session-whiteboard prepare \
      --provider codex \
      --session-id <id> \
      --cwd "$PWD" \
      --json

The JSON response contains `artifact_path`, relative to `relative_to`, and a
session-specific `viewer_url`. The URL displays only that whiteboard full-screen.
`prepare` opens that URL only when no viewer connection has been seen recently
or its keep-alive connection is gone. The daemon creates the file at:

    <session-cwd>/.session-whiteboard/<provider>/<session-key>.html

If the cwd is inside a Git repository, the first `prepare` adds the exact
`.session-whiteboard/` directory to that repository's `.git/info/exclude`.
Failure to update the exclude file is reported as a warning and does not block
artifact creation.

To open the daemon's empty landing page without registering a session:

    session-whiteboard browse

`browse` starts the managed daemon if needed and opens the empty landing page.
Use the `viewer_url` returned by `prepare` to open a specific board. Since a
generic OS browser launcher cannot reliably reuse an arbitrary existing tab,
`browse` may open another tab; `prepare` is the keep-alive-aware path.

The registry is a known-board index rather than an active/inactive session
lifecycle. The viewer does not list or switch between registry entries; each
viewer URL addresses one board. Preparing the same provider, session, and cwd
reuses the same file. Deletion is intentionally not automatic in this MVP.

To explicitly clean (delete) the HTML and its registry record:

    session-whiteboard clean --provider codex --session-id <id> --cwd "$PWD"

## Browser viewer

The first `prepare` automatically starts the daemon and opens the requested
board when no viewer connection is present. The viewer is intentionally a
single-board page with no sidebar or session switcher. The daemon watches
artifacts modified within roughly the last day at startup and adds a board when
its URL is displayed. Changes are pushed to the page for immediate reload.
Browser/OS launchers cannot force reuse of an arbitrary already-open tab across
all browser families, so keep-alive-aware preparation is the single-tab-friendly
path.

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
