pub const SESSION_TEMPLATE: &str = r##"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <!-- session-whiteboard: <title> is the sidebar title and must be updated with the document. -->
  <title>New session</title>
  <style>
    :root {
      color-scheme: light;
      --canvas: #eeece5;
      --paper: #fffdf8;
      --paper-margin: #f5f1e8;
      --ink: #293130;
      --ink-secondary: #59635f;
      --ink-muted: #7d8580;
      --marker: #176a83;
      --positive: #27754f;
      --warning: #a66d24;
      --line: rgba(41,49,48,.16);
      --line-soft: rgba(41,49,48,.09);
      --sheet-shadow: 0 1px 2px rgba(41,49,48,.08), 0 12px 28px rgba(41,49,48,.07);
      --mono: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      font-size: 12px;
    }
    * { box-sizing: border-box; }
    html, body { width: 100%; min-height: 100%; }
    body { margin: 0; overflow: auto; background: var(--canvas); color: var(--ink); -webkit-font-smoothing: antialiased; }
    main { display: grid; grid-template-rows: auto auto auto; gap: 12px; width: min(1220px, 100%); min-height: 100vh; margin: 0 auto; padding: 20px 24px; }
    .topline, .footer { min-width: 0; display: flex; align-items: center; justify-content: space-between; gap: 16px; }
    .topline { min-height: 28px; padding-bottom: 10px; border-bottom: 1px solid var(--line-soft); }
    .eyebrow, .label, .state, .mono { font-family: var(--mono); letter-spacing: .10em; text-transform: uppercase; }
    .eyebrow { color: var(--marker); font-size: 10px; font-weight: 700; }
    .state { display: inline-flex; align-items: center; gap: 6px; color: var(--positive); font-size: 10px; }
    .state::before { width: 6px; height: 6px; border-radius: 50%; background: var(--positive); content: ""; }
    .sheet { min-width: 0; min-height: 480px; display: grid; grid-template-columns: minmax(0, 1fr) minmax(190px, 28%); border: 1px solid var(--line); background: var(--paper); box-shadow: var(--sheet-shadow); }
    .focus { min-width: 0; display: grid; align-content: center; gap: 12px; padding: clamp(24px, 5vw, 64px); border-left: 4px solid var(--marker); }
    .label { color: var(--marker); font-size: 10px; font-weight: 700; }
    h1 { max-width: 800px; margin: 0; color: var(--ink); font-size: clamp(30px, 6vw, 68px); font-weight: 650; letter-spacing: -.06em; line-height: .98; text-wrap: balance; }
    .lead { max-width: 640px; margin: 0; color: var(--ink-secondary); font-size: 14px; line-height: 1.55; text-wrap: pretty; }
    .margin { min-width: 0; display: flex; flex-direction: column; gap: 18px; padding: 20px; border-left: 1px solid var(--line); background: var(--paper-margin); }
    .margin h2 { margin: 0; color: var(--ink-muted); font: 700 10px/1.2 var(--mono); letter-spacing: .10em; text-transform: uppercase; }
    .notes { margin: 0; padding: 0; list-style: none; }
    .notes li { padding: 10px 0; border-top: 1px solid var(--line-soft); color: var(--ink-secondary); font-size: 12px; line-height: 1.4; }
    .notes li:first-child { margin-top: 8px; }
    .notes strong { display: block; margin-bottom: 3px; color: var(--ink); font-weight: 650; }
    details { margin-top: auto; padding-top: 12px; border-top: 1px solid var(--line-soft); }
    summary { color: var(--marker); cursor: pointer; font: 700 10px/1.3 var(--mono); }
    details p { margin: 8px 0 0; color: var(--ink-muted); font-size: 11px; line-height: 1.45; }
    .footer { padding-top: 10px; color: var(--ink-muted); font: 10px/1.4 var(--mono); }
    .footer strong { color: var(--ink-secondary); }
    @media (max-width: 680px) {
      main { gap: 10px; padding: 14px 16px; }
      .sheet { grid-template-columns: 1fr; grid-template-rows: auto auto; }
      .focus { padding: 28px 22px; }
      .margin { border-top: 1px solid var(--line); border-left: 0; }
      .footer { display: none; }
    }
  </style>
</head>
<body>
  <main>
    <header class="topline">
      <span class="eyebrow">Session Whiteboard</span>
      <span class="state">live projection</span>
    </header>
    <section class="sheet" aria-label="Empty whiteboard">
      <div class="focus">
        <div class="label">Current focus</div>
        <h1>New session</h1>
        <p class="lead">ここから、現在の焦点に必要なコンテキストだけを一枚のボードへ再構成します。</p>
      </div>
      <aside class="margin" aria-label="Whiteboard margin notes">
        <h2>Margin notes</h2>
        <ul class="notes">
          <li><strong>focus first</strong>現在の主題を一番大きく置く</li>
          <li><strong>keep branches visible</strong>必要な枝だけを短く残す</li>
          <li><strong>discard stale context</strong>古い情報は積極的に捨てる</li>
        </ul>
        <details>
          <summary>overflow を展開</summary>
          <p>収まらない補足はここへ退避し、紙面をスクロールする長いログにしない。</p>
        </details>
      </aside>
    </section>
    <footer class="footer">
      <span><strong>one viewport</strong> · replace the draft as the focus changes</span>
      <span>paper field · margin notes</span>
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
