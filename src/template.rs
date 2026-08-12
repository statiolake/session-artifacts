pub const SESSION_TEMPLATE: &str = r##"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <!-- session-whiteboard: <title> is the sidebar title and must be updated with the document. -->
  <title>New session</title>
  <style>
    :root {
      color-scheme: light dark;
      --bg: #eef1f4;
      --surface: rgba(255,255,255,.88);
      --text: #17202a;
      --muted: #697684;
      --line: #d8dfe6;
      --accent: #6d45c4;
      --shadow: 0 18px 48px rgba(24,39,56,.10);
      font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    @media (prefers-color-scheme: dark) {
      :root { --bg:#101419; --surface:rgba(29,35,42,.92); --text:#edf2f6; --muted:#a9b4bf; --line:#38434e; --accent:#c3a9ff; --shadow:0 18px 48px rgba(0,0,0,.28); }
    }
    * { box-sizing: border-box; }
    html, body { height: 100%; }
    body { margin: 0; overflow: hidden; background: var(--bg); color: var(--text); }
    main { display: grid; place-items: center; height: 100%; padding: 32px; }
    .empty { width: min(620px, 100%); padding: clamp(28px, 6vw, 72px); border: 1px dashed var(--line); border-radius: 24px; background: var(--surface); box-shadow: var(--shadow); text-align: center; }
    .eyebrow { color: var(--accent); font-size: .72rem; font-weight: 800; letter-spacing: .15em; text-transform: uppercase; }
    h1 { margin: .45rem 0 .75rem; font-size: clamp(1.8rem, 4vw, 3.6rem); line-height: 1.05; letter-spacing: -.05em; }
    p { margin: 0; color: var(--muted); line-height: 1.6; }
  </style>
</head>
<body>
  <main>
    <section class="empty" aria-label="Empty whiteboard">
      <div class="eyebrow">Session Whiteboard</div>
      <h1>New session</h1>
      <p>ここから、現在の焦点に必要なコンテキストだけを一枚のボードへ再構成します。</p>
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
