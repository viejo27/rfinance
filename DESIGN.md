---
name: RFinance
description: A multi-currency money desk composed as a constructivist poster press — flat ink blocks, one diagonal-cut system, stamped ledger records.
colors:
  papel: "#ede6d3"
  papel-claro: "#f6f1e2"
  tinta: "#1b1813"
  tinta-2: "#4e463b"
  rojo: "#a82317"
  rojo-hondo: "#7e140c"
  rojo-grafico: "#c2331f"
  ocre: "#7e5300"
  ocre-barra: "#a97209"
  concreto: "#a8a49a"
typography:
  display:
    fontFamily: "'Anton', 'Arial Narrow', sans-serif"
    fontSize: "clamp(2.8rem, 5vw, 6rem)"
    fontWeight: 400
    lineHeight: 1
    letterSpacing: "0.01em"
  stencil:
    fontFamily: "'Saira Stencil One', 'Anton', sans-serif"
    fontSize: "1.5rem"
    fontWeight: 400
    lineHeight: 1.1
    letterSpacing: "0.02em"
  body:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.55
  label:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "0.7rem"
    fontWeight: 500
    letterSpacing: "0.09em"
  label-mini:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "0.66rem"
    fontWeight: 500
    letterSpacing: "0.09em"
  meta:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "0.72rem"
    fontWeight: 400
    letterSpacing: "0.06em"
  small:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "0.78rem"
    fontWeight: 400
  body-small:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "0.85rem"
    fontWeight: 400
  campo:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "0.9rem"
    fontWeight: 400
  ttl:
    fontFamily: "'Anton', 'Arial Narrow', sans-serif"
    fontSize: "1.05rem"
    fontWeight: 400
    letterSpacing: "0.04em"
  saldo:
    fontFamily: "'Anton', 'Arial Narrow', sans-serif"
    fontSize: "1.9rem"
    fontWeight: 400
    lineHeight: 1
  submarca:
    fontFamily: "'Saira Stencil One', 'Anton', sans-serif"
    fontSize: "1rem"
    fontWeight: 400
  monto-movil:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "1rem"
    fontFeature: "'tnum' 1"
  marca-shell:
    fontFamily: "'Saira Stencil One', 'Anton', sans-serif"
    fontSize: "1.55rem"
    fontWeight: 400
  marca-auth:
    fontFamily: "'Saira Stencil One', 'Anton', sans-serif"
    fontSize: "2.6rem"
    fontWeight: 400
    lineHeight: 1
  data-svg:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontSize: "9px"
  data:
    fontFamily: "'JetBrains Mono', ui-monospace, 'DejaVu Sans Mono', monospace"
    fontFeature: "'tnum' 1"
    letterSpacing: "0.01em"
rounded:
  zero: "0"
spacing:
  gutter: "24px"
  block-gap: "22px"
  cell-gap: "16px"
  tight-gap: "8px"
  section-gap: "56px"
components:
  button:
    backgroundColor: "{colors.papel-claro}"
    textColor: "{colors.tinta}"
    typography: "{typography.display}"
    rounded: "{rounded.zero}"
    padding: "12px 22px"
  button-hover:
    backgroundColor: "{colors.tinta}"
    textColor: "{colors.papel}"
  button-primary:
    backgroundColor: "{colors.rojo}"
    textColor: "{colors.papel}"
    typography: "{typography.display}"
    rounded: "{rounded.zero}"
    padding: "12px 22px"
  button-primary-hover:
    backgroundColor: "{colors.rojo-hondo}"
  button-danger:
    backgroundColor: "transparent"
    textColor: "{colors.tinta-2}"
    typography: "{typography.display}"
    rounded: "{rounded.zero}"
    padding: "12px 22px"
  button-danger-hover:
    backgroundColor: "{colors.rojo}"
    textColor: "{colors.papel}"
  input-field:
    backgroundColor: "{colors.papel-claro}"
    textColor: "{colors.tinta}"
    typography: "{typography.body}"
    rounded: "{rounded.zero}"
    padding: "11px 12px"
  section-band:
    backgroundColor: "{colors.rojo}"
    textColor: "{colors.papel}"
    typography: "{typography.stencil}"
    padding: "10px 48px 10px 18px"
  total-block:
    backgroundColor: "{colors.tinta}"
    textColor: "{colors.papel}"
    typography: "{typography.display}"
    rounded: "{rounded.zero}"
    padding: "18px 20px 14px"
  total-block-negative:
    backgroundColor: "{colors.rojo}"
  ledger-header:
    backgroundColor: "{colors.tinta}"
    textColor: "{colors.papel}"
    padding: "10px 14px"
  account-panel-concrete:
    backgroundColor: "{colors.concreto}"
    textColor: "{colors.tinta}"
    padding: "14px 16px 12px"
  data-chip:
    backgroundColor: "{colors.tinta}"
    textColor: "{colors.papel}"
    padding: "2px 8px"
  budget-bar-track:
    backgroundColor: "{colors.concreto}"
    rounded: "{rounded.zero}"
    height: "20px"
  auth-box:
    backgroundColor: "{colors.papel-claro}"
    textColor: "{colors.tinta}"
    rounded: "{rounded.zero}"
    padding: "34px 34px 28px"
---

# Design System: RFinance

## Overview

**Creative North Star: "The Ledger Press"**

RFinance is a money desk set as a constructivist poster press. The page ground is aged bone paper (`papel`) with real grayscale grain multiplied over the token color; on it sit flat ink-black blocks, red clipped bands, and ruled data — no cards-with-rounded-corners, no accent-on-cream fintech default. The world is owner-pinned Soviet constructivism plus utilitarian brutalism, held sober: it behaves like a registration instrument, never a novelty. The product's multi-currency reality (ARS and USD side by side, "SIN CONVERSIÓN") is composed into the layout itself — two ink totals blocks, one per currency, are the first thing on the desk.

Three voices share the stage and never trade jobs: Saira Stencil One stamps the wordmark and section titles, Anton sets numerals and button type in capitals, and JetBrains Mono carries every piece of actual data in tabular figures. One diagonal-cut system governs every angle in the build: a 26px clip on section bands, −30° corner wedges on the auth box and totals blocks, and a lone −6° tilt reserved for the rubber-stamp state. Depth is press-depth — hard 2px/3px ink frames, flat fills, and offset drop shadows only under lifted primary controls. Motion is two-frame hard cuts at 90–180ms with `steps(2, end)`; the signature moment, El Sello, stamps a newly recorded row ("ANOTADO") in one snap.

Status of this record (scan mode, captured 2026-10-14 from the shipped `/`, `/login`, `/register` surfaces): tokens and components below are evidenced in `static/css/style.css` (shared world) and `static/css/dashboard.css` (home composition). Open areas the build does not yet resolve: **i18n** — copy is Spanish-only, no EN mechanism exists yet (strings sit centralized in templates); **admin surface** — `/admin` is a placeholder that inherits the world via `style.css` (`.panel-admin`) with no composed content of its own; **backend schemas** — every ledger row, account, budget, and chart is synthetic in-memory demo data explicitly labeled "DE EJEMPLO", and no currency conversion is performed or faked. Whether income keeps ocre or moves to green is an open owner call (a single-token swap if it changes).

**Key Characteristics:**
- Five materials doing semantic jobs; one diagonal-cut family; zero radius.
- Anton giant numerals, stencil titles inside red clipped bands, mono for all data.
- Ink blocks for structure, paper for field, concrete-grey for secondary panels.
- Hard offset shadows only on lifted controls; everything else flat and framed by 2–3px ink borders.
- Step-motion only (`steps(2, end)`, 90–180ms); states cut, never ease.
- Browser surfaces (selection, caret, focus ring, scrollbar) are themed in ink/red.
- Dense ruled blocks float in wide paper margins; nothing is printed around them.

## Colors

The palette is five materials with assigned jobs, byte-exact in `:root` — colors are not decoration, they are accounting.

### Primary
- **Flag Oxide Red** (`{colors.rojo}`): the single command color — expense amounts (`.egreso`), deficit states (negative totals, negative balances), the primary action (`.btn-primario`), section bands (`.banda`), error notices, EXCEDIDO budget bars, the DEMO chip, and every interaction surface (selection fill, caret, focus ring, scrollbar-thumb hover).
- **Deep Oxide Red** (`{colors.rojo-hondo}`): press-depth of the primary — hover of red buttons only.
- **Bright Poster Red** (`{colors.rojo-grafico}`): the graphic red, reserved for marks on ink — active nav border, brand wordmark hover, sort carets on the ink header, first frame of the total-block flash.

### Secondary
- **Ochre** (`{colors.ocre}`): income and positive — ledger `.ingreso` amounts (bold) and success notices.
- **Warm Ochre** (`{colors.ocre-barra}`): the same signal as mass — budget fill bars, income bars in every chart, legend swatch.

### Tertiary
- **Bare Concrete** (`{colors.concreto}`): the secondary material — USD account panels (`.cuenta-malla`, textured) and empty budget-bar track.

### Neutral
- **Ink Black** (`{colors.tinta}`): structure and ground rules — shellbar, table headers, all borders, body text, totals blocks, chips.
- **Spent Ink** (`{colors.tinta-2}`): quiet text — labels, secondary cell content, placeholders, danger-outline resting color.
- **Aged Bone Paper** (`{colors.papel}`): page background and the light color on ink surfaces.
- **Pale Bone Card** (`{colors.papel-claro}`): raised paper surfaces — forms, panels, fields, notice fills.

### Named Rules
**The Semantic Labor Rule.** Every palette member has a job: rojo = egreso/déficit/acción primaria, ocre = ingreso/positivo, tinta/papel = estructura/ground, concreto = secundario. Never decorate with a color whose job the context contradicts — red never marks a success, ochre never marks an expense.

**The Texture-by-Blend Rule.** Material character comes from grayscale raster grain (`.textures/papel-hueso.png`, `.textures/concreto.png`, procedural with embedded provenance) multiplied over a token color — `background-blend-mode: multiply` — never from pre-tinted images or gradients. The color is always a `:root` token; the grain is always achromatic.

## Typography

**Display Font:** Anton, self-hosted woff2 (with Arial Narrow)
**Body Font:** JetBrains Mono variable 100–800, self-hosted woff2 (with ui-monospace, DejaVu Sans Mono)
**Label/Mono Font:** same mono — the label voice is mono at 0.7rem capitals, not a fourth face
**Stamp Font:** Saira Stencil One, self-hosted woff2 (wordmark and section titles only)

**Character:** a stencil gun, a poster type slamer, and a ledger typewriter in one room — industrial caps for anything shouted, machine mono for anything counted. All three are `font-display: swap`, latin subsets, no CDN.

### Hierarchy
- **Display** (Anton 400, uppercase via `.muestra`): hero numerals clamp(2.8rem, 5vw, 6rem) on totals blocks; account balances 1.9rem; poster chart headings clamp(1.6rem, 2.6vw, 2.3rem); button type 1.05rem, letter-spacing 0.04em.
- **Stencil** (Saira Stencil One 400, 1.5rem, line-height 1.1, letter-spacing 0.02em): section titles, always inside a red band; wordmark at 1.55rem on ink (60px shellbar) and 2.6rem on auth pages.
- **Body** (JetBrains Mono 400, 15px, line-height 1.55): the default voice of the entire page.
- **Data** (mono, `font-variant-numeric: tabular-nums` via `.datos`): table cells 0.88rem, dates fixed at 8.5ch, filter selects 0.85rem, chart labels 9px.
- **Label** (mono 500, 0.7rem, uppercase, letter-spacing 0.09em via `.rotulo`): field labels, strip metadata, footer; chips run hotter — 0.66rem, weight 700, letter-spacing 0.1–0.12em.
- Headings h1–h3 reset to weight 400, line-height 1.05, `text-wrap: balance`.

### Named Rules
**The Mono-Means-Data Rule.** Anything that varies row to row — ledger cells, form values, dates, counts, chart labels — is JetBrains Mono with tabular figures. Anton numerals exist only as printed posters (headline totals, balances, chart heads), never inside tables.

**The No-Kicker Rule (as practiced).** The red clipped band *is* the section title — no eyebrow line is ever printed above a heading. Small-caps mono text exists only when it does work: naming a field, carrying a date/currency/count, or declaring state (DEMO, EXCEDIDO, ANOTADO, DATOS DE EJEMPLO).

## Layout

A rigid press grid on paper. Content sits in a 1280px-max container (`.marco`, `.shell-inner`) with 24px gutters (14px at ≤640px). The sticky 60px ink shellbar carries a 4px red rule on its bottom edge — the system's signature horizon line — with brand, section nav, and user/logout in an `auto 1fr auto` grid.

Beneath it, a **marginal data strip** (`.franja-margen`): 0.72rem mono labels closed by a 2px ink hairline, values accented red — the cut date, the currency pair, the honesty warning. The command band (`.mando`) is an asymmetric `1.1fr 1.1fr 1.6fr` grid: two ink totals blocks, one real histogram, and the quick-entry form (`.captura`) as a full-width horizontal strip beneath — three-step capture on one ruled line. Then account tiles (`repeat(4, 1fr)`, gap 16px), the ledger, budget rows (`24ch minmax(0,1fr) auto`), and two-up poster charts (gap 22px). Section blocks close with 56px of bare paper; bands keep 22px below themselves and cap at 42ch wide.

Responsive is re-compaction, not reflow: at ≤1024px the mando becomes 2 columns (histogram spanning), accounts 2×2, charts single; at ≤640px everything goes single-column, the capture form reorders first (`order: -1`), and the ledger **recompacts into full cells** via `grid-template-areas` (fecha+monto / concepto / cat+cuenta) — date, concept, and amount stay visible and no column dies empty. Auth pages ignore the grid entirely: a single `place-items: center` full-viewport stack with one ≤460px box.

**The Full-Cell Recompress Rule** (from the build's doujin-catalog raise). When the grid narrows, cells re-compose as complete units — rows become stacked labeled blocks; a column is never left orphaned, and `table.libro` drops to `min-width: 0` instead of scrolling.

## Elevation & Depth

The system is flat at rest — it is paper and ink, not glass. Depth is conveyed three ways, in order of frequency: **tonal inversion** (ink blocks and ink table headers read as raised against paper; the 4px ink bar atop account tiles is the "printed over" cue), **hard framing** (3px ink borders around surfaces, 2px around controls), and exactly one shadow family: flat offset shadows with zero blur under lifted controls.

### Shadow Vocabulary
- **Control lift** (`box-shadow: 4px 4px 0 var(--tinta)`): primary buttons only; the ink wedge reads as the block's printed depth.
- **Surface lift** (`box-shadow: 8px 8px 0 var(--tinta)`): the auth box and admin placeholder panel — a sheet set on the desk.
- **Press-down** (`transform: translate(2px, 2px)` + shadow removed on `:active`): lifted controls physically seat into the paper when pressed.

### Named Rules
**The Pressed-Shadow Rule.** Offsets are hard, 0-blur, and ink-colored; they appear only under primary actions and standalone sheets. A control's `:active` state cancels its shadow and shifts 2px down-right — depth is mechanical press, never atmosphere.

## Shapes

Zero radius, everywhere, as a law: every control declares `border-radius: 0`; every surface is a rectangle. The build's real geometry is the **single-cut system** — one angle family governs all diagonals: a `--corte` (26px) polygon clip slicing the section band's top-right corner; a −30° wedge rotation used for the auth box's red corner plus its thin ink companion strip, and the totals block's red corner wedge; a −6° tilt reserved exclusively for the rubber-stamp mark (`ANOTADO`). One angle, repeated at different scales, is what makes it one world. The only other recurring silhouettes are pure-CSS triangles — the select chevron built from two 7px linear-gradients and the table sort caret built from 5–6px border tricks — no icon fonts, no SVG icon packages in chrome.

Border weights are fixed roles: 2px (the `--grosor` token) for controls, hairlines and the scrollbar track edge; 3px for framed surfaces (fields strip, tiles, table wrapper, auth box); 4px for structural rules (shellbar underline, account tile cap, footer top).

Divergence note: the direction contract scoped diagonals to section bands only; the shipped build extends the same −30°/26° grammar to corner wedges on the auth box and totals blocks. The build is canonical; the discipline is that every diagonal descends from the cut family.

### Named Rules
**The Single Cut Rule.** If an edge is slanted it came from one of exactly three sources: the 26px band clip, a −30° wedge, or the −6° stamp tilt. New angles are not added; the cut system also governs chart bar width (26px rects) and the seal — one angle across bands, bars, and sellos.

**The Zero-Radius Rule.** No corner is softened at any scale — not buttons, not inputs, not panels. Roundness is a defect in this world.

## Components

### Buttons (`.btn` base, Anton capitals, zero radius)
- **Shape:** square, 2px border, padding 12px 22px, uppercase 1.05rem, letter-spacing 0.04em, `transition: none`.
- **Primary** (`.btn-primario`): `{colors.rojo}` fill, `{colors.papel}` text, red border, `4px 4px 0` ink shadow; hover drops to `{colors.rojo-hondo}`; active presses 2px down and kills the shadow.
- **Default** (`.btn`): `{colors.papel-claro}` on paper, ink border and text; hover inverts to ink block with paper text (state cut, no fade).
- **Danger** (`.btn-peligro` / `.btn-salir`): see Destructive Isolation below.
- **Focus:** all controls share the global ring — `outline: 3px solid {colors.rojo}`, 2px offset.

### Inputs & Fields (`.campo`)
- **Style:** full-width, `{colors.papel-claro}` fill, 2px ink border, zero radius, mono 0.95rem, padding 11px 12px; labels (`.campo-label.rotulo`) sit 18px above / 6px below.
- **Selects:** native arrow removed; a 7px CSS-triangle chevron drawn with two linear-gradients at the right edge.
- **Number/date internals:** spin buttons removed (`appearance: textfield`); the date picker indicator is inked with `filter: grayscale(1) brightness(0.3)`; date fields carry `min-height: 44px` touch target. Placeholder is `{colors.tinta-2}` at full opacity.
- **In-grid variants:** the capture strip compacts fields (0.88rem, 9px 10px padding, fixed widths per field type — fecha 158px, monto 130px, moneda 96px…); radio pairs render as two-cell toggle blocks that invert to solid ink when `:checked`.

### Notices (`.aviso`)
- Base: 2px ink border, paper-card fill, 12px 14px padding. **Error:** full `{colors.rojo}` block, paper text. **Success:** ochre border and ochre text on paper — the Semantic Labor Rule applied.

### Section Band (`.banda`) — the signature title
Red block, paper stencil text inside, top-right corner sliced by the 26px clip, capped at 42ch, 22px above its section. The band *is* the heading; the same title treatment never appears outside it.

### Totals Block (`.mando-total`) — the desk centerpiece
Ink block (3px ink border, 18px 20px 14px padding) with a mono rotulo line ("TOTAL · ARS"), an Anton numeral at clamp(2.8rem, 5vw, 6rem) in tabular figures, and a quiet paper-65% sub-line of account names. A `{colors.rojo}` −30° wedge sits in the bottom-right corner. **Deficit state:** the whole block flips to red fill with red border (`.total-neg`). **Update state:** one 180ms two-frame cut from bright poster red back to ink (`.total-flash`).

### Account Panel (`.cuenta`)
Paper tile, 3px ink border, a 4px ink bar across the top, mono label / Anton balance (1.9rem) / ink data-chip for currency (0.66rem, 700, 0.1em tracking, 2px 8px). Negative balances print in `{colors.rojo}`. The **USD variant** (`.cuenta-malla`) swaps the fill to `{colors.concreto}` with `concreto.png` grain multiplied in — currency is told apart by *material*, not by hue alone.

### Ledger (`.libro`)
Framed table wrapper (3px ink, `overflow-x: auto`); ink header row, 0.68rem uppercase mono column names, sort controls as full-cell buttons with CSS-triangle carets (dim paper when idle, `{colors.rojo-grafico}` pointing up/down for active sort, `aria-sort` driven). Rows: 2px mixed hairlines, odd rows tinted at `color-mix` 55% papel, cells 9px 14px at 0.88rem tabular. Amounts are right-aligned; expenses print `{colors.rojo}`, income `{colors.ocre}` bold — and until the backend exists, demo rows prefix a red DEMO chip (`.sello-demo-h`).

### El Sello — the signature motion
The stamp moment when a row is recorded (suppressed under `prefers-reduced-motion`, in both CSS and JS): the new row cuts from a solid red block to paper over 180ms `steps(2, end)` (`sello-cae`), the affected total block cuts red→ink over 180ms (`total-brusco`), and an "ANOTADO" rubber stamp — 2px red border, red text on paper card, −6° tilt — pops from 1.9× to 1× in a single 90ms two-frame snap (`estampa-pop`) with no follow-up motion at rest.

### Budget Bar (`.presu-item` / `.presu-barra`)
One ruled row: uppercase name (24ch), a 20px bar — `{colors.concreto}` track, 2px ink border, fill in `{colors.ocre-barra}` at `min(--pct, 100%)` — and a tabular reading "usado / límite %" right. **EXCEDIDO state:** fill and border turn `{colors.rojo}` and a red uppercase chip (0.66rem/700/0.1em) closes the reading.

### Poster Charts (`.cartel`, `.histo`)
Paired 660×240 SVG bar posters (26px bars, 2px ink axis line, 9px mono labels), ink-framed tiles, Anton headline totals ("ING 6.316.000 · GAS 4.356.400"), ochre-vs-red legend, and a permanent "DATOS DE EJEMPLO" confession in the legend line.

### Navigation (`.shellbar`)
Sticky ink bar, 4px red bottom rule. Stencil wordmark (paper; hover turns `{colors.rojo-grafico}`, never background-painted). Section links are 0.72rem uppercase mono chips; the current section gets a red fill with bright-red border (`.nav-actual`); unbuilt sections ride as `aria-disabled` paper-55% ghosts that stay ghost on hover.

### User / Exit — destructive isolation
The account email (ellipsized at 24ch) and SALIR sit right, separated from everything. The exit control (`.btn-salir`) rests as a transparent outline (paper-60% border, paper text) and only fills — to paper with ink text, red focus ring — on deliberate hover/focus, the same doctrine as `.btn-peligro` (ink-2 outline → full red on hover/focus). Destructive actions never shout; they wait to be aimed at.

### Named Rules
**The Destructive Isolation Rule.** Log out and delete controls are physically separated, rendered outline-at-rest, and only resolve on deliberate hover/focus. They never carry the primary action's red fill or lift shadow at rest.

**The Step-Cut Rule (no easing).** Every state change is either instant (`transition: none` on controls) or a two-frame `steps(2, end)` cut at 90–180ms. Nothing fades, slides, or eases; under `prefers-reduced-motion` all animation is removed and stamp marks are not generated at all.

### Browser Surfaces (themed)
Selection is red-on-paper; carets and native controls accent red; the focus ring is 3px red + 2px offset globally; the 12px scrollbar is a paper track with a 2px ink left edge, an ink thumb inset by paper, and red on thumb hover.

## Do's and Don'ts

### Do:
- **Do** source every color from the five `:root` materials; if a needed value isn't in `:root`, the design isn't done yet — extend the token, then use it.
- **Do** keep the Semantic Labor assignment fixed: red for egreso/déficit/primary, ochre for ingreso/success, concrete for secondary, ink/papel for structure.
- **Do** set all data in mono with tabular figures; reserve Anton numerals for poster-scale numbers only.
- **Do** use the band as the only section-title treatment, clipped by `--corte` (26px) and capped at 42ch.
- **Do** press with hard offsets: `4px 4px 0` under primary controls, `8px 8px 0` under standalone sheets, cancel-to-`translate(2px,2px)` on active.
- **Do** cut states with `steps(2, end)` at 90–180ms or not at all, and honor `prefers-reduced-motion`.
- **Do** isolate destructive actions in outline-at-rest form (`.btn-peligro` / `.btn-salir` pattern).
- **Do** texture via multiply-blended grayscale rasters over a token color (bone paper on body, concrete on USD panels).
- **Do** label provisional data honestly — "DE EJEMPLO" / DEMO chips stay until the backend is real.
- **Do** recompact grids into full cells at 1024/640px; keep fecha, concepto, and monto visible in the mobile ledger row.

### Don't:
- **Don't** round any corner, blur any shadow, ease any motion, or float any glass — the world has none of these.
- **Don't** invent diagonal angles: only the 26px band clip, the −30° wedge, and the −6° stamp tilt exist.
- **Don't** paint success red or expense ochre; don't hand the primary-action red to anything decorative.
- **Don't** add eyebrow/kicker lines above headings or decorative small-caps that carry no field, count, or state.
- **Don't** use glyph-icon libraries — arrows and carets are CSS triangles; charts are inline SVG.
- **Don't** fade between states; two-frame hard cuts or instant swaps only.
- **Don't** fabricate financial data, conversion rates, or currency symbols — amounts arrive with their currency as a first-class label (a chip), and no cross-currency math is implied.
- **Don't** ship unthemed browser surfaces on new pages — selection, caret, focus ring, and scrollbar inherit `style.css` globally; keep it that way.
