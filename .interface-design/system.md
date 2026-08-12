# Session Whiteboard Design System

## Direction

- Human: a developer following several live agent sessions while investigating a problem.
- Job: explain the current technical subject so its necessary relationships can
  be recovered in one glance.
- Feel: a compact physical whiteboard on a developer's desk — bright paper,
  graphite notation, smart, quiet, practical, and slightly editorial rather
  than a generic SaaS dashboard.
- Density: 4px base unit; 12px body; 10–16px section rhythm; one viewport by
  default. Compactness is intentional, not an excuse to remove hierarchy.
  When content exceeds the available paper, natural scrolling is preferable to
  overlap or clipping.

## Product world

The visual vocabulary comes from paper, marker strokes, margin notes, session IDs,
cwd breadcrumbs, focus anchors, branching questions, live reload, and a sheet of
notes beside a terminal. The signature is an explanation field with marginalia:
the subject being explained occupies the broad paper surface while identity,
source references, and secondary branches stay legible but quiet in a narrow
margin.

## Tokens

```css
:root {
  --paper: #fffdf8;
  --canvas: #eeece5;
  --sheet-edge: rgba(41, 49, 48, .16);
  --ink: #293130;
  --ink-secondary: #59635f;
  --ink-muted: #7d8580;
  --marker: #176a83;
  --positive: #27754f;
  --warning: #a66d24;
}
```

Use one blue marker accent sparingly. Semantic green/amber communicate state;
they are not decoration. The board is paper-white on a warm neutral canvas;
use quiet graphite rules and one restrained sheet shadow. Do not use dark-mode
panels, gradients, or decorative card borders.

## Composition

- The viewer is a 272px navigation rail plus a fluid whiteboard pane. The rail
  serves the board; it must not become a second competing canvas.
- Keep the right pane free of persistent header chrome; the whiteboard starts at
  the top edge and owns its own title. Use 8px internal controls where needed.
- Keep the three-line session rows compact at roughly 56–60px with 4–8px
  vertical rhythm. Do not add decorative status bullets to every row; the left
  marker is reserved for meaningful unread state.
- Navigation is one chronological list; active/inactive lifecycle state is not
  a second grouping axis. File updates reorder the list immediately.
- The whiteboard has one broad explanation field and no equal grid of generic
  cards. Use a narrow annotation margin when it helps; group related anchors
  tightly, then use a deliberate larger gap between topics. Keep the normal
  state compact, but allow the paper to scroll when the alternative is hidden
  or overlapping content.
- A title and visible heading should name the subject plainly. Do not turn the
  sheet into a presentation cover with a slogan, manifesto, or oversized hero
  phrase. Information density and relationships matter more than theatrical
  emphasis.
- Prefer weight and tone over large type. Use 10px tracked labels, 12px body,
  13–16px section/value hierarchy, and tabular numbers for IDs and counters.
- Favor native semantic controls and visible focus states. A compact button is
  still at least a 40px hit target.

## Overflow and source references

The board is a current-context projection, not an archive. Secondary content may
be collapsed behind an explicit click target (for example `<details>` or a
dialog/popover), but essential content must remain readable. If the paper needs
more height, allow natural scrolling; never clip or overlap components just to
preserve a fixed viewport. The compact view should still state what is hidden.

Local source references use copy actions, not editor-specific URL schemes. Show
the relative `path:line` as the label and copy that exact string to the
clipboard on click. Keep a readable fallback label/value if the Clipboard API
is unavailable.

## Whiteboard composition

The sheet is the product surface, not a dashboard card grid. Use a warm paper
surface (`--paper`) with a broad explanation field, a vertical marker rail or
other restrained focal edge, and marginalia for anchors/status. The margin may
contain compact notes and copy actions, but it must not become a second equal
canvas.
The default split is approximately 72/28 rather than three equal columns.

## Provenance

This project design system was informed by
[interface-design](https://github.com/Dammyjay93/interface-design) by
Dammyjay93, MIT licensed.
