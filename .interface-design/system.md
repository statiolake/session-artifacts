# Session Whiteboard Design System

## Direction

- Human: a developer following several live agent sessions while investigating a problem.
- Job: identify the active session and recover the current focus in one glance.
- Feel: a dense graphite workbench — smart, quiet, practical, and slightly
  editorial rather than a generic SaaS dashboard.
- Density: 4px base unit; 12px body; 10–16px section rhythm; one viewport by
  default. Compactness is intentional, not an excuse to remove hierarchy.

## Product world

The visual vocabulary comes from session IDs, cwd breadcrumbs, focus anchors,
branching questions, live reload, and a sheet of notes pinned beside a terminal.
The signature is a focus rail: the current topic leads, while session identity,
source references, and secondary branches stay legible but quiet.

## Tokens

```css
:root {
  --canvas: #0d1115;
  --panel: #11171d;
  --panel-raised: #171f26;
  --inset: #0a0e12;
  --ink: #e8edf0;
  --ink-secondary: #a8b4bc;
  --ink-muted: #71808b;
  --line: rgba(232, 237, 240, .10);
  --line-soft: rgba(232, 237, 240, .06);
  --focus: #76d5df;
  --positive: #8bd5a3;
  --warning: #e3b978;
}
```

Use one cyan focus accent sparingly. Semantic green/amber communicate state;
they are not decoration. Use borders-only depth with quiet translucent lines;
do not mix in dramatic shadows or gradients.

## Composition

- The viewer is a 272px navigation rail plus a fluid whiteboard pane. The rail
  serves the board; it must not become a second competing canvas.
- Keep the viewer chrome at roughly 48px high and use 8px internal controls.
- The whiteboard has one dominant focus, no page scroll in its normal state,
  and no equal grid of generic cards. Group related anchors tightly, then use
  a deliberate larger gap between topics.
- Prefer weight and tone over large type. Use 10px tracked labels, 12px body,
  13–16px section/value hierarchy, and tabular numbers for IDs and counters.
- Favor native semantic controls and visible focus states. A compact button is
  still at least a 40px hit target.

## Overflow and source references

The board is a current-context projection, not an archive. Content that cannot
fit should be collapsed behind an explicit click target (for example `<details>`
or a dialog/popover) rather than causing the main page to scroll. The compact
view must still state what is hidden.

Local source references use copy actions, not editor-specific URL schemes. Show
the relative `path:line` as the label and copy that exact string to the
clipboard on click. Keep a readable fallback label/value if the Clipboard API
is unavailable.

## Provenance

This project design system was informed by
[interface-design](https://github.com/Dammyjay93/interface-design) by
Dammyjay93, MIT licensed.
