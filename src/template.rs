pub const SESSION_TEMPLATE: &str = r##"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <!-- session-whiteboard: <title> is the sidebar title and must be updated with the document. -->
  <title>New session</title>
  <style>
    :root {
      color-scheme: dark;
      --canvas: #0d1115;
      --surface: #11171d;
      --surface-raised: #171f26;
      --ink: #e8edf0;
      --ink-secondary: #a8b4bc;
      --ink-muted: #71808b;
      --line: rgba(232,237,240,.10);
      --line-soft: rgba(232,237,240,.06);
      --focus: #76d5df;
      --positive: #8bd5a3;
      font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      font-size: 12px;
    }
    * { box-sizing: border-box; }
    html, body { width: 100%; height: 100%; }
    body {
      margin: 0;
      overflow: hidden;
      background: var(--canvas);
      color: var(--ink);
      -webkit-font-smoothing: antialiased;
    }
    main {
      display: grid;
      grid-template-rows: auto 1fr auto;
      gap: 16px;
      width: min(1120px, 100%);
      height: 100%;
      margin: 0 auto;
      padding: 20px 24px;
    }
    .topline {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
      min-height: 28px;
      padding-bottom: 10px;
      border-bottom: 1px solid var(--line-soft);
    }
    .eyebrow, .label {
      color: var(--ink-muted);
      font: 600 10px/1.2 ui-monospace, SFMono-Regular, Menlo, monospace;
      letter-spacing: .10em;
      text-transform: uppercase;
    }
    .eyebrow { color: var(--focus); }
    .state {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      color: var(--ink-muted);
      font: 500 10px/1.2 ui-monospace, SFMono-Regular, Menlo, monospace;
    }
    .state::before {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: var(--positive);
      content: "";
    }
    .focus {
      display: grid;
      align-content: center;
      gap: 10px;
      min-width: 0;
      padding-left: 16px;
      border-left: 2px solid var(--focus);
    }
    h1 {
      max-width: 760px;
      margin: 0;
      color: var(--ink);
      font-size: clamp(26px, 5vw, 54px);
      font-weight: 650;
      letter-spacing: -.055em;
      line-height: .98;
      text-wrap: balance;
    }
    .lead {
      max-width: 600px;
      margin: 0;
      color: var(--ink-secondary);
      font-size: 13px;
      line-height: 1.5;
      text-wrap: pretty;
    }
    .anchors {
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 8px;
      align-self: end;
    }
    .anchor {
      min-width: 0;
      padding: 10px 12px;
      border: 1px solid var(--line);
      background: var(--surface);
    }
    .anchor strong {
      display: block;
      margin-top: 5px;
      overflow: hidden;
      color: var(--ink);
      font-size: 12px;
      font-weight: 600;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .footer {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding-top: 10px;
      border-top: 1px solid var(--line-soft);
      color: var(--ink-muted);
      font: 11px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace;
    }
    @media (max-width: 620px) {
      main { gap: 12px; padding: 14px 16px; }
      .anchors { grid-template-columns: 1fr; }
      .anchor { padding: 8px 10px; }
      h1 { font-size: clamp(28px, 12vw, 46px); }
    }
  </style>
</head>
<body>
  <main>
    <header class="topline">
      <span class="eyebrow">Session Whiteboard</span>
      <span class="state">live projection</span>
    </header>
    <section class="focus" aria-label="Empty whiteboard">
      <div>
        <div class="label">Current focus</div>
        <h1>New session</h1>
      </div>
      <p class="lead">ここから、現在の焦点に必要なコンテキストだけを一枚のボードへ再構成します。</p>
    </section>
    <footer class="footer">
      <span>one viewport · replace the draft as the focus changes</span>
      <div class="anchors" aria-label="Whiteboard principles">
        <div class="anchor"><span class="label">01</span><strong>focus first</strong></div>
        <div class="anchor"><span class="label">02</span><strong>keep branches visible</strong></div>
        <div class="anchor"><span class="label">03</span><strong>discard stale context</strong></div>
      </div>
    </footer>
  </main>
</body>
</html>
"##;

pub fn render_session_template(title: &str) -> String {
    let safe_title = escape_html(title);
    SESSION_TEMPLATE
        .replace(
            "<title>New session</title>",
            &format!("<title>{safe_title}</title>"),
        )
        .replace("<h1>New session</h1>", &format!("<h1>{safe_title}</h1>"))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
