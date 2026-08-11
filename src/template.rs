pub const SESSION_TEMPLATE: &str = r##"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <!-- session-artifacts: <title> is the sidebar title and must be updated with the document. -->
  <title>New session</title>
  <style>
    :root {
      color-scheme: light dark;
      --bg: #f4f6f8;
      --surface: rgba(255,255,255,.86);
      --surface-strong: #fff;
      --text: #17202a;
      --muted: #637080;
      --line: #dbe1e7;
      --accent: #216eaa;
      --accent-soft: #e5f2fc;
      --ok: #19734a;
      --shadow: 0 18px 48px rgba(24, 39, 56, .10);
      font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    @media (prefers-color-scheme: dark) {
      :root { --bg:#101418; --surface:rgba(28,34,40,.92); --surface-strong:#1b2229; --text:#edf2f6; --muted:#a7b2bd; --line:#36404a; --accent:#79b8e8; --accent-soft:#16354a; --ok:#75d5a3; --shadow:0 18px 48px rgba(0,0,0,.28); }
    }
    * { box-sizing: border-box; }
    body { margin:0; background:var(--bg); color:var(--text); line-height:1.65; }
    .artifact { max-width: 1040px; margin: 0 auto; padding: 48px 28px 72px; }
    .artifact-header { padding: 28px 0 30px; border-bottom:1px solid var(--line); }
    .eyebrow { color:var(--accent); font-size:.74rem; font-weight:750; letter-spacing:.13em; text-transform:uppercase; }
    h1 { margin:.45rem 0 .6rem; font-size:clamp(2rem, 4vw, 3.4rem); line-height:1.08; letter-spacing:-.04em; }
    h2 { margin:0; font-size:1.2rem; letter-spacing:-.015em; }
    h3 { margin:1.5rem 0 .5rem; font-size:1rem; }
    p { margin:.65rem 0; }
    a { color:var(--accent); }
    .lede { max-width: 760px; color:var(--muted); font-size:1.08rem; }
    .grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(240px,1fr)); gap:16px; margin:24px 0; }
    .card { background:var(--surface); border:1px solid var(--line); border-radius:18px; padding:20px; box-shadow:var(--shadow); }
    .card h2 { display:flex; align-items:center; gap:9px; }
    .card h2::before { content:""; width:9px; height:9px; border-radius:50%; background:var(--accent); }
    .callout { border-left:4px solid var(--accent); background:var(--accent-soft); border-radius:0 14px 14px 0; padding:14px 18px; margin:18px 0; }
    .status { display:inline-flex; align-items:center; gap:7px; color:var(--ok); font-size:.88rem; font-weight:700; }
    .status::before { content:""; width:8px; height:8px; border-radius:50%; background:currentColor; }
    code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
    code { background:rgba(127,127,127,.14); border-radius:5px; padding:.1em .35em; }
    pre { overflow:auto; border:1px solid var(--line); border-radius:12px; background:var(--surface-strong); padding:16px; }
    table { width:100%; border-collapse:collapse; margin:14px 0; }
    th, td { text-align:left; vertical-align:top; border-bottom:1px solid var(--line); padding:10px 8px; }
    th { color:var(--muted); font-size:.82rem; }
    details { border-top:1px solid var(--line); padding:12px 0; }
    summary { cursor:pointer; font-weight:700; }
    .muted { color:var(--muted); }
  </style>
</head>
<body>
  <main class="artifact">
    <header class="artifact-header">
      <div class="eyebrow">Session artifact</div>
      <h1>New session</h1>
      <p class="lede">このHTMLを、セッションの現在地・理解・未解決事項を構造化して表示するライブノートとして使います。</p>
    </header>

    <section class="grid" aria-label="Current state">
      <article class="card">
        <h2>現在の結論</h2>
        <p class="muted">このセクションを、いま最も重要な理解の要約に更新してください。</p>
      </article>
      <article class="card">
        <h2>未解決事項</h2>
        <p class="muted">確認が必要な問い、判断待ちの事項、前提をここに整理してください。</p>
      </article>
    </section>

    <section class="card">
      <h2>詳細</h2>
      <div class="callout">回答・質問・調査結果は、会話本文ではなく、このHTMLの適切な位置へ反映してください。</div>
      <p class="muted">この本文はエージェントがテーマに合わせて再構成します。見出し、リンク、表、図、折りたたみ、コード参照などを使ってください。</p>
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
