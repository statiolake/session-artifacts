pub const SESSION_TEMPLATE: &str = r##"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <!-- session-whiteboard: <title> names this board in the browser document. -->
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
    main { display: grid; width: min(1220px, 100%); min-height: 100vh; margin: 0 auto; padding: 16px 20px; }
    .eyebrow, .label, .state, .mono { font-family: var(--mono); letter-spacing: .10em; text-transform: uppercase; }
    .eyebrow { color: var(--marker); font-size: 10px; font-weight: 700; }
    .state { display: inline-flex; align-items: center; gap: 6px; color: var(--positive); font-size: 10px; }
    .state::before { width: 6px; height: 6px; border-radius: 50%; background: var(--positive); content: ""; }
    .sheet { min-width: 0; min-height: 420px; display: grid; grid-template-columns: minmax(0, 1fr) minmax(190px, 28%); border: 1px solid var(--line); background: var(--paper); box-shadow: var(--sheet-shadow); }
    .focus { min-width: 0; display: grid; align-content: start; gap: 12px; padding: clamp(20px, 4vw, 32px); border-left: 4px solid var(--marker); }
    .label { color: var(--marker); font-size: 10px; font-weight: 700; }
    h1 { max-width: 800px; margin: 0; color: var(--ink); font-size: clamp(20px, 3vw, 30px); font-weight: 650; letter-spacing: -.03em; line-height: 1.1; text-wrap: balance; }
    .lead { max-width: 640px; margin: 0; color: var(--ink-secondary); font-size: 13px; line-height: 1.5; text-wrap: pretty; }
    .margin { min-width: 0; display: flex; flex-direction: column; gap: 12px; padding: 16px; border-left: 1px solid var(--line); background: var(--paper-margin); }
    .margin h2 { margin: 0; color: var(--ink-muted); font: 700 10px/1.2 var(--mono); letter-spacing: .10em; text-transform: uppercase; }
    .note { margin: 0; color: var(--ink-secondary); font-size: 12px; line-height: 1.45; }
    @media (max-width: 680px) {
      main { padding: 12px 14px; }
      .sheet { grid-template-columns: 1fr; grid-template-rows: auto auto; }
      .focus { padding: 28px 22px; }
      .margin { border-top: 1px solid var(--line); border-left: 0; }
      .footer { display: none; }
    }
  </style>
</head>
<body>
  <main>
    <section class="sheet" aria-label="Empty whiteboard">
      <div class="focus">
        <div class="label">Whiteboard</div>
        <h1>New session</h1>
        <p class="lead">このページを、現在説明している技術的な対象と、その関係が一覧できるボードに置き換えてください。</p>
      </div>
      <aside class="margin" aria-label="Whiteboard margin notes">
        <h2>Ready</h2>
        <p class="note">この下書きを現在の説明に置き換えてください。<code>&lt;title&gt;</code> はサイドバーの名前です。</p>
      </aside>
    </section>
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
