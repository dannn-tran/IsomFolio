# IsomFolio Design System

This document is the single source of truth for visual and interaction decisions across the app. Absorbs the welcome-ui-spec. Apply these rules consistently when building new screens or modifying existing ones.

**Normative.** This doc and the code are kept in lockstep: any change that diverges from a rule here updates *both* — either the code conforms, or the rule is revised in the same change set. A rule that no longer matches shipped behaviour is a bug in this doc.

**Who it's for.** A photographer culling and organising large shoots. The app optimises for **speed of cull** (flag/rate/label/compare without friction) and **findability** (the user can locate any capability without prior knowledge). Resolve design questions in that light.

---

## Philosophy

Principles, in **priority order** — when two conflict, the higher one wins:

1. **Clarity & discoverability.** Every feature has a path a first-time user finds *without prior knowledge* — a visible control, a menu entry, a hover tooltip, or a one-time hint. "It's a keyboard shortcut" or "you have to right-click" is not, by itself, discoverability. **Quiet ≠ hidden.**
2. **Legible & reachable.** Text and hit-targets stay above the floors in *Density floor* below, regardless of how dense a layout wants to be. Never rely on opacity or colour *alone* to convey meaning.
3. **Compact and information-dense.** Minimise chrome. Every pixel should carry content or breathing room, not decoration — but not at the cost of (1) or (2).
4. **Quiet by default.** Use spacing, typography weight, and opacity to express hierarchy. Reserve borders, backgrounds, and colour for meaning (selection, error, active state), not layout structure.
5. **Progressive disclosure.** Show the simplest view first. Expand only when the user asks (criteria panel, detail panel, modals).
6. **Reversible and confirmed.** Destructive or irreversible actions require explicit confirmation. Single-click navigation and selection are safe and immediate.

The historical failure mode of this app is (3)/(4) silently overriding (1)/(2): minimal collapsing into hidden, dense collapsing into illegible. The ordering exists to stop that.

### Density floor

Density may not breach these, even when "compact" argues otherwise:

- **Body / interactive text** ≥ `TEXT_SM` (11 px). `TEXT_XS` (10 px) is for non-interactive micro-labels only.
- **Interactive hit-target** ≥ 24 px in its smaller dimension (pad small glyph controls to reach it). **Icon-only buttons** standardise *above* this floor at a uniform **`ICON_BTN` = 28 px square** (a comfortable pointer/coarse target without the bloat a 44 px touch target would force on a dense desktop layout). They are never sized ad-hoc per call site — see *Buttons*.
- **Meaning is never opacity-only.** A dimmed/greyed state must also differ by another channel (icon, label, position) so it survives low-contrast displays and colour-blindness.

### Discoverability rules

- **Every icon-only / glyph control MUST have a hover tooltip** (`styles::tip`). A bare glyph with no label and no tooltip is a defect.
- **Every action has an off-row, discoverable path.** Right-click / gestures are the *fast* path, never the *only* path: each must also be reachable via a menu entry and/or be documented in the `?` help panel (which lists gestures and right-click menus, not just key bindings).
- **New capability checklist:** before a feature ships, name the path a first-time user finds it through. If the only answer is "they already knew the key" or "they happened to right-click," add a visible/menu/tooltip path first.

#### Discoverability inventory

Every significant action must have at least one discoverable path beyond right-click. This table is the canonical checklist — a blank cell in the menu or keyboard column is not automatically a defect (drag and `?` help panel satisfy the requirement for some actions), but it must be a conscious, documented decision.

| Action | Context menu | Menu bar | Keyboard | Other visible path |
|---|---|---|---|---|
| Flag Pick | Tile | Photo → Flag Pick | `P` | Loupe HUD button |
| Flag Reject | Tile | Photo → Flag Reject | `X` | Loupe HUD button |
| Unflag | Tile | Photo → Unflag | `U` | Loupe HUD button |
| Set colour label | Tile | Photo → Label | `6`–`9` | Loupe colour swatches |
| Add to Album | Tile | — | `B` (target album) | Drag-to-album; `?` help panel |
| Set Target Album | Album | — | — | `?` help panel |
| Show in Finder | Tile | Photo → Show in Finder | — | — |
| Locate… (orphaned) | Tile (orphaned only) | — | — | Tile "Missing" banner |
| Copy to Folder… | Tile | Photo → Copy to Folder… | — | — |
| Export Album… | Album row | — | — | Copies all present files in the album to a chosen folder |
| New group | Album row | — | — | Albums-section header `library`-plus glyph |
| Move to Group | Album row | — | — | Album context menu → submenu of groups + Ungrouped |
| Rename / Delete group | Group row | — | — | Group context menu (delete keeps the albums) |
| Import XMP | Tile | Photo → Import XMP | — | — |
| Write XMP Sidecars | — | Photo → Write XMP Sidecars | — | — |
| Export Metadata (CSV) | — | Photo → Export Metadata (CSV)… | — | — |
| Open in Loupe | Tile | View → Loupe | `Space` · double-click | — |
| Compare | — | Photo → Compare | `C` | — |
| Review Stacks | — | View → Review Stacks | `R` | Guided full-bleed pass over every stack in the view |
| Keep this, reject rest (stack) | Tile (single, stacked) | — | — | Picks the clicked frame, rejects its stack-mates |
| Reject whole stack | Tile (single, stacked) | — | — | Rejects every frame in the stack |
| Expand / Collapse stack | Tile (single, stacked, while collapsed) | — | — | Click the tile's `⧉ N` badge |
| Delete (soft) | — | Edit → Delete | `Del` / `Backspace` | — |
| Restore from Deleted | Deleted-view tile | — | — | — |
| Move to Trash… | Deleted-view tile | — | — | — |
| Empty Deleted… | — | — | — | Status bar button |
| Sync Folder | Folder | — | `Cmd+R` | — |
| Add Folder | — | Catalog → Add Folder… | — | Sidebar `+` button |
| Remove from Library… | Folder | — | — | — |
| Rename Album | Album | — | — | — |
| Duplicate Album | Album | — | — | — |
| Delete Album… | Album | — | — | — |
| New Smart Album | — | Photo → New Smart Album from Filters… | — | Criteria panel Save button |
| Edit Smart Album Criteria | Smart album | — | — | `?` help panel |
| Find People | — | Photo → Find People | — | — |
| Re-cluster All Faces | — | Photo → Re-cluster All Faces | — | — |
| Rename Person | Person card | — | — | — |
| Open Settings | — | — | `Cmd+,` | Gear icon (menu bar) |
| Toggle Info Panel | — | View → Toggle Info Panel | — | — |
| Toggle Filters | — | — | `F` | Sidebar "Filters" section header |
| Toggle Help | — | — | `?` | `?` icon (menu bar) |
| Hide Rejects | — | View → Hide Rejects | `\` | — |

**Noted gaps:** Restore from Deleted, Move to Trash, Set Target Album, Edit Smart Album Criteria, and Rename Person are context-menu-only. All must appear in the `?` help panel under their respective sections as compensation. Remove from Library is context-menu-only for folders — it must be in the Catalog menu when a folder is selected. Add "Remove from Library…" to the Catalog menu spec.

---

## Colour Tokens

Defined in `src/view/styles.rs`. Never hardcode raw `Color` literals for semantic roles — use these constants.

| Token | Usage |
|---|---|
| `BG_SIDEBAR` | Sidebar, detail panel background |
| `BG_GRID` | Main content area, welcome screen background |
| `BG_STATUSBAR` | Status bar strip |
| `BG_CRITERIA` | List-layout column-header strip background |
| `BG_MODAL` | Modal card background |
| `BG_TILE_LOADING` | Tile placeholder background (thumbnail not yet loaded) |
| `FG` | Primary text, icons |
| `FG_DIM` | Secondary labels, metadata, section headers |
| `FG_MUTED` | Placeholder text, deeply de-emphasised content |
| `ACCENT` | Selected state, primary action buttons |
| `ALBUM_HOVER` | Album drag-target hover highlight |
| `STAR_GOLD` | Filled star rating |
| `ERR` | Validation errors, destructive prompts |
| `DANGER` | Destructive action button background |
| `BORDER` | Panel borders, dividers, input field borders |

Modal scrims and fullscreen compositing overlays (loupe, scrim) use literal `Color` values defined at call site — they are one-off compositing layers, not semantic roles.

---

## Typography

No decorative fonts. All text uses the default iced system font. Font size constants are defined in `src/view/styles.rs`.

| Token | px | Role | Colour |
|---|---|---|---|
| `TEXT_DISPLAY` | 36 | App hero title | `FG` |
| `TEXT_TITLE` | 20 | Modal / section titles | `FG` |
| `TEXT_LG` | 14 | Primary labels, action buttons | `FG` |
| `TEXT_BASE` | 13 | Body text, file names | `FG` |
| `TEXT_MD` | 12 | Compact body, button labels, sidebar section headers | `FG` / `FG_DIM` |
| `TEXT_SM` | 11 | Metadata, chips, menu item shortcuts | `FG_DIM` |
| `TEXT_XS` | 10 | Error copy, micro labels | `ERR` / `FG_DIM` |
| `TEXT_STAR` | 18 | Star rating icons only | `STAR_GOLD` / `FG_DIM` |

Do not create new size tiers. Pick the closest existing token.

---

## Iconography

Two distinct icon mechanisms, by origin:

- **Unicode glyphs** — compact inline controls and status marks (flags `✓ ✕ ○`, `⚡ ⟳ ★ ● ◎ ⧉`, layout toggles `▦ ≡`, sort `▲ ▼`, grid tile-size `− +`, confirm `✓ ✕`). Rendered as text in the system font; no asset needed. Keep using these for compact inline controls and for chip/row status — not for the structural disclosure/add controls below. (The *loupe* zoom buttons are the exception — they use the Lucide `zoom-in`/`zoom-out` magnifier icons, not bare `− +`, so the control reads unambiguously as *image* zoom.)
- **SVG line icons** — for *navigation destinations* (sidebar rows/headers) **and the structural control glyphs that sit beside them** (disclosure chevrons, the `+` add action). [Lucide](https://lucide.dev) (ISC), embedded under `assets/icons/`, rendered via `view/icons.rs` (`icon(Icon, Color)`) at `ICON_SIZE` (15 px) and **tinted single-colour** through `svg::Style.color`, so a glyph adopts its row's state colour (`FG_DIM` at rest, brighter/`Color::WHITE` when selected). Never multi-colour; never emoji (colour emoji clash with the quiet dark UI).

An icon resource is **not** a "decorative font" — the *Typography* rule ("default system font only") governs text, not iconography. Adding a new icon: drop the Lucide SVG in `assets/icons/`, add an `Icon` variant. Use sparingly — icons aid recognition on a *few* high-level destinations; do not sprinkle them on every row (e.g. folder-tree leaves and import batches stay text-only).

**Disclosure & add glyphs — Lucide SVG, one family.** Expand/collapse and add affordances render through `icon_btn_svg` (a Lucide glyph centred in the `ICON_BTN` square), *not* unicode — so a header's chevron and `+` read as peers of its leading section icon instead of the heavier unicode triangles/plus they replaced. Horizontal disclosure (sidebar sections, folder tree) is **`chevron-down` expanded / `chevron-right` collapsed**; vertical collapse (the task panel) is **`chevron-down` collapse / `chevron-up` expand**; the add action is **`plus`**. Chevrons point toward the motion. Keep the family consistent — do not reintroduce unicode `▾ ▸ ▴` for these controls (mixing SVG and unicode for the same gesture reads as two different controls). **Feedback exception:** an SVG's tint can't follow button hover state the way text colour can, so `icon_btn_svg` shows hover/press as a faint background (vs `icon_btn`'s tint-brighten) — this is the one icon-only button that carries a hover box, and it is deliberate.

---

## Spacing

All spacing uses a 4 px base unit (`UNIT = 4.0` in `styles.rs`). Each token name is its Tailwind-style multiplier — `SPACE_2` = 2 × 4 = 8 px. Fractional tokens follow Tailwind convention (`SPACE_1_5` = 1.5 × 4 = 6 px).

| Token | px | Typical use |
|---|---|---|
| `SPACE_0_5` | 2 | Micro gaps (star rating row, very tight list spacing) |
| `SPACE_1` | 4 | Tiny nudges, icon-text gap, small inline padding |
| `SPACE_1_5` | 6 | Compact field padding, tight row spacing |
| `SPACE_2` | 8 | Standard item gap, list spacing |
| `SPACE_2_5` | 10 | Action row spacing, button groups |
| `SPACE_3` | 12 | Panel padding, sidebar padding |
| `SPACE_4` | 16 | Section gap, modal field group gap |
| `SPACE_5` | 20 | Welcome screen vertical pad |
| `SPACE_6` | 24 | Modal padding, major section gap |

To adjust global density, change `UNIT` in `styles.rs` — all tokens scale proportionally.

---

## Button Variants

Defined in `src/view/styles.rs`. Use the right variant for the action's weight.

| Style function | When to use |
|---|---|
| `icon_btn_style` | Icon-only buttons in section headers and toolbars (`+`, chevrons, etc.). No background — text colour brightens on hover. No box, no padding box. |
| `ghost_btn_style` | Secondary text actions, toggles, chip/filter in off state |
| `active_chip_style` | Primary action (enabled), toggled-on state |
| `danger_btn_style` | Destructive confirm (delete, remove) |
| `quiet_btn_disabled_style` (welcome.rs local) | Primary action when preconditions unmet |

**Rule:** any button that is icon-only (single glyph, no text label) MUST use `icon_btn_style`. `ghost_btn_style` is only for buttons that carry a text label or are inside a content region where a subtle box hover is expected.

**Icon-button helpers (sizing is centralised — do not hand-size).** Every icon-only button routes through one helper in `styles.rs` so the clickable square is uniformly `ICON_BTN` (28 px) with an `ICON_GLYPH` (16 px) mark, regardless of where it lives:
- `icon_btn(glyph, msg)` — the default: `icon_btn_style`, tint brightens on hover.
- `icon_btn_color(glyph, msg, color)` — fixed glyph colour when the colour carries meaning (a colour-label swatch, an `ERR` ×).
- `icon_btn_styled(glyph, msg, style)` — for icon-only *toggles* that swap to `active_chip_style` when on (Grid/List switch, colour-label swatch active fill).
- `icon_btn_svg(Icon, msg)` — the mark is a tinted **Lucide SVG** (not a unicode glyph), centred in the same `ICON_BTN` square. Used for the disclosure chevrons and the `+` add action (→ *Iconography*), so they match the leading section icons' weight. Hover/press feedback is a faint background, since an SVG tint can't follow button state.

Unicode `×`/`✕`/`✓`/`⚙`/zoom `−`/`+` use the text helpers; **disclosure chevrons and the section `+` use `icon_btn_svg`** (see *Iconography → Disclosure & add glyphs*). Override `.height(…)`/`.width(…)` only to fit a host band (menu bar height, the folder-tree chevron's fixed alignment slot) — never to shrink the target below `ICON_BTN`. New icon buttons MUST use a helper, not a bare `button(text(glyph)).style(icon_btn_style)`.

Buttons without `on_press` are visually disabled. Do not use `ghost_btn_style` for a primary action when an `active_chip_style` primary button exists nearby.

---

## Component Patterns

### Entity row anatomy

The governing question for any row is: **what is the row's primary verb?** That determines whether actions belong inline.

- **Navigation rows** — the primary verb is *select / navigate to*. Sidebar folders, sidebar albums, recent-catalog items, person (face-cluster) cards. The user clicks the row to go somewhere; rename/delete/etc. are occasional secondary actions.
- **Management rows** — the row has *no* navigation verb; its reason to exist is to be acted upon. The Tag Browser table is the canonical case: a tag row exists so you can rename, apply, or delete that tag. The actions *are* the content.

The rule differs by kind:

**Navigation rows:**
- Show: name + optional read-only status badges (photo count, ⚡ smart indicator, scan spinner, dirty dot).
- Do **not** embed action buttons — at rest, on hover, or on selection. No inline ×, ✎, •••, `+`.
- **One sanctioned exception — the dirty dot.** It is an always-present status badge (not hover-revealed) whose *only* action is to resolve the very state it reports: click it to sync that folder. It stays because it doesn't compete with the row's select verb (it sits in the trailing badge cluster, not the label), it is never a generic action menu, and the same action is also reachable off-row (`Cmd+R`, context-menu **Sync Folder**). A second inline trigger on a nav row needs the same three properties or it doesn't belong.
- Context menu (right-click / Ctrl+Click) is the **fast** path to row actions.
- The context menu is **not the only** path: every action must *also* be reachable off-row — a menu entry (Photo menu mirrors tile actions; Catalog menu mirrors folder/album actions for the selected entity) and/or the `?` help panel's "Right-click menus" section. (See *Discoverability rules* in Philosophy.)

**Management rows:**
- Inline action controls are expected, not forbidden — they are the row's primary verbs. Hiding the only actions of a management surface behind invisible right-click is an anti-pattern (no affordance, no discoverability).
- Keep them lightweight (text/`ghost_btn_style`, not heavy buttons) and stable (no hover-reveal — the action is always present because it is always the point).
- A management surface is a *dedicated* view opened deliberately (modal/panel), never the always-on sidebar. If you find yourself wanting inline buttons on a sidebar row, it's a navigation row — use the context menu.

**Why navigation rows stay quiet:** embedding buttons conflates display with action on a high-frequency scan surface, competes with the primary click target, and hover-reveal adds clutter + a small hit area while still being hidden. **Why management rows don't:** the action is the content; an off-row-only path on a surface built for those actions is hostile. **Why also off-row (nav):** a right-click-only action with no cue is undiscoverable — surface the action's *existence* elsewhere.

Do not add action buttons to **navigation** rows. Do not make a navigation-row action reachable *only* by right-click. Inline actions on a dedicated **management** surface are correct.

**Bulk actions on navigation entities** use a *transient batch bar*, not per-row buttons. Cmd/Ctrl-click toggles entities into a selection (accent outline); a plain click cancels the selection and navigates. While a selection is active, a batch bar appears above the grid with the bulk action (e.g. the People view's "Name & merge" for face clusters). This keeps rows quiet at rest while still surfacing bulk operations discoverably — the bar only exists when it applies. Mirrors the photo grid's multi-select model.

### Row heights

Two row height constants exist to express the hierarchy between containers and items:

| Constant | px | Used for |
|---|---|---|
| `ALBUM_ITEM_HEIGHT` | 32 | Albums (manually curated — slightly taller than folders) |
| `FOLDER_ITEM_HEIGHT` | 28 | Folders (file system entries — compact, utilitarian) |

Folder rows are intentionally more compact. Do not normalise them to `ALBUM_ITEM_HEIGHT`.

**Groups** (recursive containers holding albums *and other groups*) sit inside the Albums list. A group header row uses `ALBUM_ITEM_HEIGHT` — it reads as a peer of the album rows it groups, not a folder. Anatomy: collapse chevron · group glyph (Lucide `library`) · name · right-aligned album count. The collapse chevron sits in the **same fixed `CHEVRON_W` slot as the folder tree** (not the wider `ICON_BTN`), so a group header and a folder row share one disclosure column and the group glyph lines up over its nested rows. The tree renders **recursively** (`render_group_block`): each nesting level adds one `CHEVRON_W` disclosure-column of indent, child groups list before the group's own albums (folder-tree order), and ungrouped albums follow at the top level. A group's depth is its `parent_id` chain; deleting a group re-homes its children (albums *and* sub-groups) to the top level. **Plain click** (or `Ctrl`-less left-press) toggles collapse; a press that travels starts a drag to re-nest the group; **right-click / Ctrl+Click** opens its context menu (New Album / New Group inside / Rename / Delete Group). All press paths route through one message so Ctrl+Click is a true right-click alias, never a stray collapse.

The Albums section header carries **one** add control: a single `+` glyph opening a small **New Album / New Group** menu (`ContextMenuTarget::AlbumsAdd`). Two near-identical plus glyphs (the former `library`-plus + `+`) read as ambiguous — and a folder-flavoured glyph mislabels a group as a folder. A single labelled menu is unambiguous and is the standard collections-"+" pattern. The `+` is a real button, so its left-press is *captured* (never fires the global `MousePressed` that dismisses menus), letting it open the menu the same way right-click entity menus do.

**Sidebar confirm rows** (inline Delete confirmation for an album or group) put the prompt in a `Fill`+`clip` container so the **Cancel / Confirm buttons stay pinned and on-screen** in the narrow (220 px) sidebar. A bare prompt + `Space::Fill` let a long string push Confirm off the right edge where it couldn't be clicked — keep confirm prompts short (`Delete "Name"?`), not sentences.

### Sidebar row classes

The sidebar has exactly **two** row kinds; the split is *"does this hold a child list or not?"* Each class is internally uniform — section identity comes from the label, never from per-row styling.

**Class A — section header (holds a collapsible list):** **Filters, Folders, Albums, Imports**. Built by the `section_header` helper. Anatomy: **leading section icon** (Lucide SVG, `FG_DIM`, see *Iconography*) · label (`TEXT_MD`/`FG_DIM`) · flexible spacer · right-aligned action glyphs (`+`, only where the section has an add action — Lucide `plus` via `icon_btn_svg`) · optional inline status ("Syncing…") · **trailing collapse chevron** (Lucide `chevron-down` expanded / `chevron-right` collapsed via `icon_btn_svg`). The collapse chevron sits at the **trailing (right) edge** — the disclosure convention — *not* the leading edge: a left chevron pushes the section icon out of the shared icon column and reads as stray chrome. Headers carry the **same horizontal padding** (`[0, SPACE_1]`) and **icon→label spacing** (`SPACE_1_5`) as Class-B nav rows, so every section icon and nav-row icon lines up in **one vertical column**. **The header band toggles collapse.** The icon · label · spacer region is a single click target (`mouse_area` → `ToggleSidebarSection`) — the obvious affordance is the section name, not only the far-right chevron (Fitts). The chevron stays at the trailing edge as the **glanceable open/closed indicator** and toggles the same section (a redundant control, not chrome). The **action glyphs** (`+`) sit *outside* this hit area as their own buttons, so clicking `+` adds rather than collapsing. Toggling collapse never changes selection or navigates. Collapse state is per-section, in-memory (not persisted across restart). Action buttons stay to the left of the chevron regardless of collapse. **Filters** is a Class-A header whose body is not a row list but the filter controls, and it does not live in the scrollable section list — it is **pinned to the sidebar bottom** (→ *Filters (sidebar section)*); a `●` after its label marks active filters.

**Class B — nav row (a single destination, no child list):** **All Photos**, **People**, **Deleted**, and each **import batch** under the Imports header. Built by the `nav_row` helper. Anatomy: **leading icon** (Lucide SVG, `FG_DIM` at rest / `Color::WHITE` when selected) · label (`TEXT_BASE`/`FG`) · right-aligned count (`TEXT_SM`/`FG_MUTED`, omitted when 0) · full-row click; **accent background fill when selected** (the same fill folders/albums use — selection is never colour-only, per *Density floor*). No chevron, no inline action buttons (per *Entity row anatomy*: nav rows carry no embedded actions — e.g. re-clustering lives in the Photo menu, not on the People row). Height `ALBUM_ITEM_HEIGHT`. Class-B rows are not collapsible — there is no list to hide. Import batches are nav rows *without* an icon (`icon: None` reserves the icon column so labels still align) — keeping leaf rows quiet, like folder-tree leaves.

**Ordering & grouping — two stacked panels.** The sidebar is split into an **upper navigation panel** (scrollable, "where to look") and a **lower filtering panel** (pinned to the bottom, "how to narrow"). Top→bottom: catalog-name title · **search box** · `──` · *[scroll:]* **All Photos** · Folders · Albums · People · `──` · Imports · Deleted · *[pinned bottom:]* `──` · **Filters** · *(Open Catalog… footer)*. The **search box** is pinned above the scroll region (always reachable, never scrolls away), fenced by a `sidebar_divider()`. The **navigation panel** fills the middle and owns the whole scroll region (`FillPortion(2)`). The **Filters panel** is pinned at the bottom: its header (with `●` active marker) is always visible; collapsed by default, expanded it takes a bounded share (`FillPortion(1)`) and scrolls internally, so opening it *squeezes* — never *hides* — the navigation above.

This placement matches the browsing mental model: **pick where to look (collection), then narrow it (filters)** — not the reverse. Search owns the fast cross-cutting narrow at the top (pinned, never scrolls); Filters is the deliberate structured narrow at the bottom, near nothing it competes with. Putting Filters above the collections (the former layout) buried the just-selected folder/album under a wall of criteria — navigation and filtering were fighting for the same vertical space and the same "what does a click do" semantics. Splitting them into two pinned-bounded panels resolves both. Within the navigation panel the first block is user/library content; the final block is **system collections** (app-generated: Imports, Deleted), fenced by a second `sidebar_divider()`. Content sections are otherwise separated by **spacing alone** (the iconned header already marks a new group); the dividers carry meaning (search vs nav; content vs system; nav vs filters) rather than decorating every boundary (*Quiet by default*). **All Photos** is the catalog-level home (default `SidebarItem::AllFiles`, always-present way back to the whole catalog). **Deleted** shows only when something is soft-deleted; **People** only when a face cluster or inference engine is present; **Imports** only when batches exist.

### Folder tree

Folders render as a navigable **tree**, not a flat list, and as a **forest** when top-level dirs diverge (separate volumes are sibling roots; there is no `/`/`Users` ghost parent). *(How the tree is built and where expansion/scan-depth state live → `architecture.md`, UI rendering.)*

- **Anchored at the library root** → the tree starts at the folders the user added (the deepest common ancestor of the added folders on a drive), not the filesystem root — so the `/Users/me` prefix above the content is hidden. Folders on different drives are separate roots.
- **Compact folders (breadcrumb)** → below the anchor, a run of single-child pass-through folders (no own photos) renders as **one row** with the segments joined by muted `/` separators — `a / b / c` — VS Code style. Each segment is **separately clickable** (navigates to that folder); the intermediate names stay visible rather than being hidden. A plain folder is just a one-segment breadcrumb, so it reads like an ordinary row.
- **Expand/collapse** → a leading chevron (`▸` collapsed, `▾` expanded), `TEXT_LG`, `icon_btn_style`, in a fixed-width slot so all labels align regardless of whether a row has children. The chevron toggles the row's deepest folder and is a separate control — it does not change selection.
- **Indentation** → each depth level adds `SPACE_2` of leading space (tight, for compactness). The breadcrumb clips to the row width; an overflow tooltip shows the full chain.
- **Count** → the photo count includes the folder *and* all descendants, not just direct children.
- **Selection** → clicking a segment selects that folder and loads its photos recursively; the whole row highlights when any of its segments is selected, with the selected segment in `WHITE`.
- **Context menu** (right-click / Ctrl+Click) → **Sync Folder**, **Add Folder…** (opens the folder picker anchored at the clicked folder — Capture One style), and **Remove from Library…** (plus **Remove Missing Files…** when the folder has orphans).
- **Dirty dot** → an accent `●` after the folder name means the watcher saw structural changes on disk (files added / removed / renamed) that have not been applied. The catalog is never mutated silently. The dot **is itself the one-click sync trigger** — clicking it (pointer cursor, tooltip *"Click to sync new files"*) syncs that folder and clears the dot; `Cmd+R` and the right-click **Sync Folder** still do the same. This is the single sanctioned inline trigger on a sidebar row (see *Entity row anatomy* — it is a status dot first, action second, not a hover-revealed button). (Pure content edits to an already-tracked file are not structural: they just refresh that file's thumbnail, no dot.) It is also listed in the `?` help panel under Folders.
- **Offline** → a library root on an unplugged drive shows an eject glyph `⏏` and its rows dim to `FG_MUTED`. Auto-clears on reconnect (polled). Offline is a recoverable state, never confused with deleted; its photos still appear (cached thumbnails) carrying an `Offline` tile banner (`WARN`), the same slot the `Missing` banner uses for a file gone while its drive is present.

### Context menu

Implemented as a `stack` overlay anchored to the cursor position. No scrim — context menus are non-blocking.

**Trigger:** right-click or Ctrl+Click anywhere on a sidebar entity or grid tile (Ctrl+Click is a right-click alias). There is no overflow button.

**Style:**
- Background: `BG_MODAL`
- Border: `BORDER`, 1 px, 6 px radius
- No scrim (non-blocking; dismissed by click-outside or Escape)
- Item height: 32 px
- Item text: `TEXT_MD`, `FG`
- Hover state: ghost background (α 0.10)
- Separator: 1 px `BORDER` line, 4 px vertical margin
- Destructive item: `ERR` text + leading `⚠` glyph (`TEXT_MD`). Two-channel signalling — colour is insufficient alone (see *Density floor*). The `⚠` glyph makes destructive items distinguishable without colour perception.

**Dismiss:** click outside the menu, press Escape, or select any item. Selecting an item closes the menu *unconditionally* — every leaf action is dispatched wrapped in `Msg::MenuAction(Box<Msg>)`, whose single handler clears `context_menu` before running the inner message. Individual handlers must **not** rely on (or re-implement) menu-closing. The only exception is the submenu-opening toggle (`Add to Album ▶` / `Merge into ▶`), which is dispatched raw so it keeps the menu open.

**Context menu contents by entity type:**

| Entity | Menu items |
|---|---|
| Folder | Sync Folder · (Remove Missing Files…, when orphans present) · — · Remove from Library… |
| Manual album | Rename · Duplicate · Set/Clear Target Album · Move to Group ▶ · Copy to Folder… · — · Delete… |
| Smart album | Rename · Duplicate · Edit Criteria · Move to Group ▶ · Copy to Folder… · — · Delete… |
| Group | New Album · New Group inside · (Select Albums, when non-empty) · — · Rename · Copy to Folder… · — · Delete Group… (albums & sub-groups are kept) |
| Grid tile (single) | Open in Loupe · Add to Album ▶ · Delete · _(if stacked:)_ — · Keep this, reject rest · Reject whole stack · (Expand/Collapse stack, while collapsed) · — · Import XMP metadata · (Import Apple Finder tags, macOS) · Show in Finder / Locate… · Copy to Folder… |
| Grid tile (multi-select) | Add to Album ▶ · — · Import XMP metadata · (Import Apple Finder tags, macOS) · Show in Finder · Copy to Folder… |
| Person (face card) | Rename · Merge into ▶ |
| Recent catalog item | Open · Remove from Recents |

Each of these also has an off-row path (Photo / Catalog menus) or is listed in the help panel — see *Entity row anatomy*.

Ellipsis (…) in a menu item label signals the action has a secondary step (rename → inline input; delete → inline confirm; add to album → submenu). Items without ellipsis execute immediately.

**"Copy" vs "Export".** Plain file-to-folder duplication is labelled **"Copy to Folder…"** — for a single photo, a multi-selection, an album, *and* a group. It copies the bytes verbatim with no re-encoding or transform, so "Export" (which implies processing) would mislead. Reserve **"Export"** for actions that genuinely produce a derived artefact, e.g. "Export Metadata (CSV)…".

**Copy-to-Folder structure & safety.** All four entry points feed one structured copy (`CopyEntry { rel, src }` → `fileops::copy_into_dir`):
- **Loose photos** (grid selection) copy flat into the chosen folder (`rel` empty).
- **An album** copies into a sub-folder named after the album (`<dest>/<album>/…`).
- **A group** mirrors its structure: `<dest>/<group>/<album>/…`, one sub-folder per album — this is the multi-album copy.
- Folder names are run through `fileops::sanitize_component` (illegal chars → `-`, empty → `Untitled`).
- **Always non-destructive:** existing directories are merged into, never cleared; a name collision never overwrites — `copy_into_dir` adds a numeric suffix (`photo.jpg` → `photo (1).jpg`). Never reintroduce a bare `std::fs::copy` into a fixed destination path for these flows.

**Confirmation from context menu:** destructive items (Remove from Library…, Delete…) close the context menu and replace the entity row with `confirm_action_row` inline. The confirm pattern itself is unchanged — only the trigger mechanism moves from an embedded button to the menu.

**Submenus (Add to Album ▶):** render as a second context menu panel to the right of the parent item. List all manual albums by name. Selecting one adds the dragged/selected files immediately (safe, no confirm). If no albums exist, show a disabled "No albums yet" item.

**Submenus (Move to Group ▶):** the album-row submenu lists **Ungrouped** (lift to top level) · every group by name · **New Group…** (opens the inline create-group input and files the album(s) into the group the moment it's confirmed). A `✓` marks the album's current group — *only* when acting on a single album; with a multi-selection the marker is omitted (the albums may sit on different groups). When `selected_albums` holds more than one and the clicked album is a member, the parent item pluralises to **"Move N albums to Group ▶"** and the chosen group applies to the whole group; otherwise it acts on the clicked album alone. All moves are safe (catalog-only, reversible), so no confirm.

### Selection states

Use a translucent `ACCENT` overlay (α ≈ 0.22–0.28) for selected items. Do not change text colour on selection unless contrast demands it. Use a 3 px `ACCENT` ring for grid tiles. Use a rounded background fill for list items (sidebar albums, recent catalogs).

**Grid tile selection (Finder / Lightroom semantics).** A **plain** click selects only the clicked tile; **Cmd/Ctrl**-click toggles a tile in/out (disjoint multi-select); **Shift**-click selects the contiguous range from the anchor (clicking back toward the anchor shrinks it). The one non-obvious rule is the **plain click on a tile that's already part of a multi-selection**: it must **collapse the selection to just that tile** — but the collapse is **deferred to mouse-up**, and only when *no drag happened in between*. This keeps press-and-drag of the whole group working (drag N selected tiles onto an album) while still letting a plain click cancel a multi-selection down to one. Doing the collapse on mouse-*down* would break group drag; never not collapsing at all leaves a stale multi-selection (the bug this rule fixes). Modifiers held at release (Cmd/Shift/Ctrl) suppress the collapse.

**Unified drag-and-drop model.** Every drag — photo tiles, album rows, and group headers — runs through **one** state machine, so there is a single place to reason about start, threshold, hover, and drop. State is one `App::drag: DragContext { current: Option<Drag>, hover: Option<DropTarget> }`. A press builds `Drag { payload, start, cursor, past_threshold: false }` as a *click candidate*; `MouseMoved` flips `past_threshold` once it passes `DRAG_THRESHOLD` (and snapshots the dragged photo set). `DragPayload` is `Photos { origin_idx, ids } | Albums { pressed } | Group { pressed }`. Drop targets are pushed by the droppable widget itself via **one** message `HoverDrop(Option<DropTarget>)` → `DropTarget::{Album, Group}`, not inferred from geometry. The global `MouseReleased` resolves *everything* through `resolve_drop(payload, target)`, guarded by the `drop_allowed(payload, target)` **compat matrix** — the single source of truth for which payload may land on which target: `Photos→Album`, `Albums→Group`, `Group→Group` (nesting). A release that never passed the threshold is a plain click (album → navigate; group → toggle collapse; tile → the deferred grid-selection collapse). **Drop zones are mounted per active payload** — during a photo drag only manual-album rows wrap themselves in a `HoverDrop` `mouse_area`; during an album *or* group drag every group block does (`dragging_onto_group`) — so there is never cross-talk between target kinds, and smart albums are simply never mounted as photo targets. Nested group blocks mount their zones inside their parent's; the deepest under the cursor wins `drag.hover` (innermost `on_enter` fires last). A `Group→Group` drop that would form a cycle is rejected with status feedback (`group_move_would_cycle`), leaving the tree intact.

**Drag ghost.** While a drag is past the threshold, a small `ACCENT` pill trails just below-right of the cursor as the top `stack` layer — a count for photos/albums (`"3 photos"` / `"2 albums"`), or the group's name for a group drag. iced has no native drag image, so it is a passive overlay positioned by padding (no `mouse_area`, never captures events). It is the always-visible "something is being dragged" signal; the status bar still narrates the target ("drop on \"Trip\"", "Nesting \"2024\" inside \"Clients\"").

**Album multi-select & drag-to-group (sidebar).** Albums carry a *second* selection axis, separate from `selected_item` (the one navigated view): `selected_albums`, built by **Cmd-click** on album rows (each highlights with the same `ACCENT` fill + ring as a navigated album). This exists only to file several albums onto a group at once. Because an album row needs **press-down** to start a drag, the row is a bare `mouse_area` (no inner `button`) whose `on_press` captures the press; click-vs-drag resolves on `MouseReleased` per the unified model above. The dragged set follows the grid rule — **the whole `selected_albums` group if you grabbed a member, otherwise just the pressed album** (`dragged_albums`). `Cmd`-click toggles membership without navigating; `Ctrl`-click still opens the context menu; `Esc` or navigating away clears the selection. The drop target is the **whole expanded group block** — header *and* its nested album rows — not the header alone: while an album is mid-drag the block is wrapped in a `HoverDrop` `mouse_area`, so a release anywhere over the group (or over a collapsed group's header) files the album. The group **header** is the hit-highlight (`ALBUM_HOVER` + `ACCENT` border + glyph→`ACCENT`, driven by `drag.hover == Some(Group(id))`) — the group's identity row lights up to mark the target; releasing off any group cancels and keeps the selection. Both manual and smart albums participate (as drag *sources*; only manual albums are photo drop *targets*).

**Cmd+A expands an album selection to its siblings** (Finder-within-a-folder semantics). While `selected_albums` is non-empty and no text input is focused, `Cmd/Ctrl+A` selects every album sharing a container — group, or the ungrouped top level — with anything already selected (`album_siblings`; a selection spanning two groups grabs both). With no album selection, `Cmd+A` keeps its normal meaning (select all grid tiles). A group's **Select Albums** context-menu item is the discoverable, no-modifier equivalent (selects exactly that group's albums).

**Create-in-place under a group.** A group's **New Album** and **New Group inside** context items open the inline name input **nested under that group** (indented like its siblings, the group auto-expanded), and the new entity is filed/nested there on confirm (`pending_album_group` / `pending_group_parent`) — mirroring "New Folder inside this folder." The top-level **+ → New Album / New Group** still create ungrouped entities, with their inputs at the top of the Albums list. Top and nested inputs never both show: the top input is suppressed whenever its pending-parent is set.

**Stack collapse, expand & cull.** The toolbar **⧉ Stack** toggle is the *global* collapse (one sharpest-rep tile per stack, `collapse_bursts`). On top of that, each stack is independently expandable in place via `expanded_bursts` (session state; threaded into the query as `SearchQuery::expanded_bursts`, never persisted). The affordance is the tile's **`⧉ N` badge itself** — while collapsed it gains a `▸`/`▾` arrow and becomes a click target (its own `mouse_area` captures the press so the tile underneath is *not* selected). This is the one sanctioned case of a status glyph doubling as a control: it stays in place, costs no extra chrome, and only activates while collapsed. Expanded members are **real tiles** — full selection, flags, ratings, and loupe step-through — so reviewing a burst never requires turning the global toggle off. Flipping the global toggle clears all per-stack expands. Resolving a stack is a one-click context action on the rep tile: **Keep this, reject rest** (anchor → Pick, the rest → Reject) or **Reject whole stack**, both written atomically in core (`set_stack_flags`) and undoable — the undo snapshot covers hidden siblings even though they aren't in the visible list.

**Stacking run feedback.** Stacking is a background pass with no progress chrome of its own, so the **Settings → Stacking** panel carries the signal: an at-rest readout (`StackStats` — "N frames hashed · M stacks across K frames", or "Not yet stacked"), and a **Re-stack now** button that disables to **Stacking…** while `stacking_in_flight`. A *user-initiated* re-stack also announces its result on the status bar; *auto* passes (which fire repeatedly as thumbnails generate) refresh the readout silently — no status churn. Deliberately **not** a background-task-panel entry: `bg_push` force-opens the panel and auto-stack runs too often for that to be anything but noise. (A future embedding-clustering pass — one slow pass, not repeated — *would* warrant a task-panel entry.)

**Review Stacks (`ViewMode::ResolveStacks`).** A guided pass that resolves *every* stack in the current view, one at a time — for when a shoot has many bursts to grind through. It is a **view mode**, not a modal or a docked panel: same surface class as Loupe and Compare (full content width so frames show large, sidebar dropped, toolbar/`Esc` retained, **no scrim** — a sustained flow, not an interruption). Built on Compare's full-res decode path generalised to N frames. Entered with `R` or **View → Review Stacks**; it gathers the view's stacks **uncollapsed** (so all members are present), capture-ordered, each pre-marking its **sharpest** frame as the default keeper. Per stack: click frames to toggle keeper (kept = `ACCENT` ring + "✓ Keep"; reject = dark scrim + "✕ Reject"), then **Keep selected & Next ›** (keepers → Pick, rest → Reject, undoable) or **Skip**; `‹ Previous` steps back. Auto-advances; finishing or `Esc` returns to the grid and reloads. The grid selection is cleared on entry so stray flag/delete keys are inert — keeper choice is by click here. The final stack's flag write is chained to the exit reload (`ResolveFinished`) so the grid never shows a stale last result.

### Reject display (dim, don't remove)

A rejected grid tile is **dimmed in place** (dark scrim, α ≈ 0.55) rather than removed — the grid keeps its continuity during a cull and a reject stays one click from being un-rejected, instead of vanishing and reflowing the layout. Exceptions: a *selected* or *being-dragged* reject is shown normally (you're acting on it), and when the view is filtered to **rejects only** they're shown normally (you're reviewing them deliberately). "Hide Rejects" / the flag filter still *removes* rejects entirely when the user explicitly wants them gone — dimming is the default in-place state, hiding is the opt-in.

### Delete is virtual (Deleted folder)

**Delete never touches the file on disk.** "Delete" (the `Del`/`Backspace` key, the Photo menu, or "Delete Rejected Photos") sets a virtual `is_deleted` flag in the catalog: the photo drops out of every normal view and collects in a virtual **Deleted** sidebar entry (shown only when non-empty, with a count). **Restore** (right-click in the Deleted view) clears the flag — instant and lossless, because the row never left the catalog (ratings/tags intact). There is no on-disk trash folder and no file move. (Inside a manual album, `Del` instead unlinks from the album.) Implementation invariant — the flag survives re-sync — is in `architecture.md`.

**Move to Trash** is the one action that touches disk: "Move to Trash…" (Deleted-view context menu, on a selection) or "Empty Deleted…" (status bar) moves the actual files to the **OS Trash / Recycle Bin** (`trash` crate; platform name via `os_trash_name()`) and removes the catalog rows. The *file* is recoverable from the OS trash until it's emptied; the *catalog edits* (rating, tags, album membership) are dropped and do not survive a re-import — that loss is the permanent consequence. Because it's a batch removal with permanent catalog-edit loss, it uses a **modal dialog** (centred card + scrim), not an inline confirm: the scrim blocks accidental click-through, and the single dialog serves both triggers (the context-menu item closes the menu and opens it; the status-bar button opens it directly). See *Confirmation pattern* and *Modal dialogs* for the carve-out.

### Confirmation pattern

Two-step for destructive ops: first trigger (context menu item) → inline confirm row appears on the entity (prompt in `ERR`, Cancel + Confirm buttons). `confirm_action_row()` helper in styles.rs.

Single-step for safe ops: primary button directly triggers action.

The trigger for a destructive op is always the context menu item, never a persistent inline button.

**The confirm co-locates with its trigger, never the status bar.** A context-menu trigger closes the menu and shows its confirm inline on the entity; a status-bar button confirms beside itself. A confirm rendered far from where the user clicked (e.g. a context-menu action whose confirm appears in the status bar) is a bug.

**Modal carve-out — the disk-touching removal only.** Reversible / catalog-only confirms stay inline. The *single* action that moves files off disk (Move to Trash) instead uses a **modal dialog**: it's a batch operation whose catalog-edit loss is permanent, and the scrim guards against an accidental click-through. This is the only confirmation permitted to use a modal; do not let it spread to reversible ops. Dialog: Esc / Cancel / scrim-click all cancel, default focus is Cancel, `danger_btn_style` on the confirm.

### Destructive action inventory

Every destructive action must be listed here with its reversibility and confirmation copy. A new destructive action not in this table is a design omission. See also *Context menu → Style* for the `⚠` + `ERR` two-channel treatment on context menu items.

| Action | Entry point(s) | Reversibility | Confirmation copy |
|---|---|---|---|
| **Delete** (soft) | `Del`/`Backspace` · Photo menu · context menu | ✅ Reversible — Restore clears the flag | None — safe immediate action |
| **Remove from Album** | `Del`/`Backspace` in album view | ✅ Reversible — drag back | None |
| **Remove from Library…** | Folder context menu | ✅ Catalog-only; files untouched | "Remove [folder] from library? Files stay on disk. [Cancel] [Remove]" |
| **Remove Missing Files…** | Folder context menu (orphans present) | ⚠️ Catalog rows gone; files were already absent | "Remove N missing file(s) from catalog? [Cancel] [Remove]" |
| **Delete Album…** | Album context menu | ✅ Album removed; photos stay in library | "Delete album '[name]'? Photos stay in the library. [Cancel] [Delete Album]" |
| **Move to Trash…** | Deleted-view tile context menu | ⚠️ File recoverable in OS Trash until emptied; **catalog edits lost** | **Modal** (title "Move to Trash"): "N photo(s) will be moved to the Trash. Their ratings and tags are removed from the catalog." `[Cancel]` (default) `[Move to Trash]` (`danger_btn_style`). Trash/Recycle Bin term via `os_trash_name()` |
| **Empty Deleted…** | Status bar button | ⚠️ File recoverable in OS Trash until emptied; **catalog edits lost** | Same modal as above — one dialog serves both triggers; N reflects all photos in Deleted |

**Confirm button style rule:** `danger_btn_style` for irreversible file-deleting operations. Standard `ERR`-coloured text with `ghost_btn_style` for reversible catalog-only operations. Cancel always appears to the left of the confirm button; default focus is Cancel.

### Locate… (missing file recovery)

When a file is orphaned (drive present but file moved or renamed externally), it shows a **Missing** banner on its grid tile and is eligible for Locate…

**Trigger:** right-click the tile → "Locate…". Opens a system file picker pre-scoped to the file's last-known folder.

**On confirmation:** the catalog row's `path` and `folder` update to the chosen file's normalised path; `is_orphaned` clears; thumbnail regenerates. All metadata (ratings, tags, flags) is preserved.

**On cancel:** no change — the file stays orphaned.

**Edge case:** if the chosen file is already tracked under its new path, surface an inline error: "This file is already in the catalog." Do not create a duplicate row.

### Disabled primary button

Show the button at reduced opacity (`FG_MUTED` text, α 0.04 background) without `on_press`. Never hide a primary button — always show its position so the user understands what is needed to unlock it.

### Metadata portability (write-back & export)

The catalog is the working store, but metadata can be made portable for preservation/interop:

- **Write XMP Sidecars** (Photo menu) writes a standard XMP sidecar (`<file>.xmp`) carrying rating, label, and Dublin Core title/caption/creator/subjects(=tags)/rights — readable by Lightroom, Bridge, Capture One, and exiftool. It **never modifies the original image**; the sidecar sits beside it (matching the read path's `with_extension("xmp")`).
- **Export Metadata (CSV)** dumps the catalog metadata for the selection (or the whole current view) to a CSV file via a save dialog — a portable, app-independent record.

### Descriptive metadata (detail panel)

Above the tag section, the detail panel has editable **Title · Caption · Creator · Copyright** text fields (Dublin Core / IPTC). Each is a labelled `text_input`; **Enter saves** (`SaveDetailField`). In batch selection the fields start blank and saving applies to **all** selected files (apply-a-rights-block-to-a-selection). These are stored in the `metadata` table and preserved across re-sync (imported-once invariant). Creator is stored as a JSON array (multi-author capable) though the field edits a single value for now. *(Full-text indexing of these + write-back to XMP are separate items.)*

### Tag section (detail panel)

The detail panel's tag section has four parts, in order:

1. **Confirmed tags** — each as a chip with `render_tag_name` (hierarchy dimming) + remove button.
2. **Autocomplete suggestions** — appear when the tag input has text. Ranked: prefix match first, then substring, with leaf-segment-aware matching. Max 5 chips.
3. **Recent tags row** — last 8 tags used this session, filtered to exclude tags already on the file. One-click apply. Label "Recent" in `FG_DIM`.
4. **Tag input** — `text_input` with `on_submit` → `AddDetailTag`.

(AI auto-tagging and its "pending/suggested" tag staging were removed; there is no AI confidence badge.)

**Batch mode**: When multiple files are selected, the detail panel shows "{n} photos selected" and the intersection of shared tags. Adding/removing tags applies to all selected files.

### Tag browser

Modal overlay (440 px wide, 420 px scrollable list). Two display modes:

- **No filter**: tree view. Tags sorted alphabetically via `BTreeMap`. Child tags indented (`INDENT_PX = 16` per level), showing only the leaf segment. Virtual group headers shown in `FG_DIM` for parent prefixes that aren't tags themselves.
- **With filter**: flat list. All matching tags shown with full path, indented by depth.

The Tag Browser is the canonical **management surface** (see *Entity row anatomy*): tag rows carry inline actions by design, because acting on the tag *is* the row's purpose. Each tag row has: leaf name, file count, "+" (apply to current file), "Rename", "Delete" (`ERR`), all `ghost_btn_style`. Rename and delete have inline confirm states.

### Shortcut help panel

Modal overlay (340 px), triggered by `?` key. Keyboard bindings grouped by category (Navigation, View, Culling, Tagging). Each row: key combo in `ACCENT` (100 px column) + label in `FG`. Dismissed by Escape or ✕ button.

Bindings defined declaratively in `keybinds.rs` — the help panel iterates the same data. Adding a shortcut = one line in the binding table.

### Error display

Inline, near the cause. Use `ERR` colour. Short copy. No modal for validation errors.

**Surface failures with a resolution action, never a silent broken state.** When an operation fails in a way the user can fix, the message must (1) say what failed in plain language, (2) say *why*, and (3) offer the concrete next step — a button to the place the fix lives, not just prose. The canonical case is the **loupe full-res load failure**: when the original file can't be decoded (commonly a macOS permission denial on a protected folder like `~/Downloads`), the loupe would otherwise fall back to a silently pixelated thumbnail with no zoom. Instead it overlays an explanatory card (see *Non-happy states*) carrying the reason and the fix: an **Open Privacy Settings** primary button (`active_chip_style`, macOS deep-link to Full Disk Access) when the cause is permission, plus **Show in Finder** to locate the file. The failure reason is classified (permission / missing / unsupported), not a raw error string. A blocked or unreadable file must never present as merely "blurry".

### Non-happy states

Every content area must define what it shows when it isn't full of content. Named patterns:

| State | Pattern |
|---|---|
| **Empty — no library** | Onboarding call-to-action centred in the grid: heading (`TEXT_MD`/`FG`) + one line of guidance (`TEXT_SM`/`FG_DIM`) + a primary button (`active_chip_style`). Never a bare "nothing here". e.g. "No photos yet — Add a folder to start your catalog" + **Add Folder…**. |
| **Empty — filtered/album** | Quiet single line: "No photos in this view" (`TEXT_BASE`/`FG_DIM`). The user created this state, so no CTA. |
| **Loading — thumbnails** | Tile placeholder (`BG_TILE_LOADING`) per tile until ready; aggregate progress in the task panel. Never block the grid. |
| **Capability absent** | When a feature needs an uninstalled extension (e.g. People with no engine), the view explains it and links to where to enable it (Settings → Extensions) rather than showing an empty or broken control. |
| **Content can't load (loupe)** | When the loupe's full-res decode fails, a centred card (`BG_MODAL`, 1 px `BORDER`, 10 px radius) overlays the image area: `⚠` (`WARN`) · heading "Can't open this photo" (`TEXT_BASE`/`FG`) · filename + reason (`TEXT_SM`/`FG_DIM`) · an action row. Permission denials show **Open Privacy Settings** (`active_chip_style`); all failures show **Show in Finder**. Never leave a failed decode as a silent pixelated thumbnail (→ *Error display*). |

The distinction matters: an *empty library* is a dead end the app must help the user out of (CTA); an *empty filter result* is an expected, user-created state (quiet line, no nag).

### Background task panel

One panel renders **every user-facing long-running process** through a single uniform `TaskView` shape — no per-process special-casing (sync, thumbnails, face clustering/embedding, album copy/move, extension install). Bottom-right; collapsible to a "N tasks" pill. **Two deliberate exemptions, each with a stated reason:** *auto-stacking* (status-line + Settings readout instead — `bg_push` force-opens the panel and auto-stack fires per-thumbnail, so a panel entry would be pure noise; see *Stacking run feedback*) and *best-effort cache hygiene* (the catalog-open thumbnail sweep — invisible by design, surfaces only to stderr on failure, because it is housekeeping the user neither triggered nor needs to watch). Anything else that runs while the user can do other work **must** appear here — that is the transparency contract.

- **In progress** → title + a 2px bar: determinate fills proportionally, indeterminate floats a centred segment ("working, amount unknown"). Optional detail line (`FG_MUTED`) carries counts/ETA.
- **Completed** → the row does **not** silently vanish. It lingers as `✓ <title>` (`ACCENT`) with its final detail for `COMPLETED_TTL` (4s), then auto-expires via a 1s tick that only runs while completions are present. This is the app-wide completion signal — visible even when the user has navigated away from the originating view. *Ambient, high-frequency work (thumbnails) is exempt — it would spam toasts on every folder switch; only discrete or long operations report completion.*
- **Failed** → row stays in `ERR` with the message and a manual ✕ dismiss (no auto-expire — errors must be read).

### Modal dialogs

Use `stack` overlay: base layer + semi-opaque scrim (`Color { r:0, g:0, b:0, a:0.55 }`) + centred modal card. Modal card: `BG_MODAL` background, 10 px radius, 24 px padding, fixed width (≈ 420 px). Reserve modals for focused multi-field task flows (e.g. New Catalog). Do not use modals for simple toggles or confirmations — **with one carve-out: the Move to Trash confirm** (see *Confirmation pattern* → modal carve-out). `modal_with_backdrop` builds an inert-scrim modal; `modal_with_backdrop_dismiss(modal, msg)` makes a scrim-click emit `msg` (used by Move to Trash to cancel).

---

## Accessibility

The app is `Theme::Dark` only; these still apply.

- **Contrast.** Body/interactive text must clear a perceptible contrast margin against its background — don't pick an `FG_*` token purely for aesthetic dimming if the result is hard to read. `FG_MUTED` is for genuinely de-emphasised, non-essential text only, never for anything the user must read or click.
- **Never opacity-alone for meaning.** A disabled, dimmed, or active state must differ on a second channel too (a glyph, a label, a position, a ring) — not just alpha. (See *Density floor*.)
- **Keyboard focus must be visible.** Any focusable control shows a clear focus indicator; keyboard navigation must never leave the user unable to see what's focused.
- **Hit targets** follow the *Density floor* (≥ 24 px). Pad small glyph controls rather than shrinking the target.
- **Motion.** Keep transitions short and functional; avoid motion that conveys required information (it should be a nicety, not the message).

These are aspirational where the framework limits us (iced's focus-ring support is partial); treat them as the target, and don't *remove* an existing affordance that serves them.

---

## Layout Patterns

### Menu bar

Custom horizontal bar (height 26 px, `BG_STATUSBAR` background). Left side: content-operation menus (`Catalog`, `Edit`, `Photo`, `View`). Right side: persistent icon-only buttons (`?` → shortcut help, `⚙` → Settings), each with a tooltip.

**Rule:** Menu tabs are for catalog/content/photo operations. App-level config (Settings) lives on the gear icon button, not in a menu tab. There is no Help tab — keyboard shortcuts are accessed via `?` icon or the `?` key. The menus collectively are the **off-row discoverable path** required by *Entity row anatomy* — folder/album actions for the selected sidebar entity live in the Catalog menu; photo/selection actions live in the Photo menu.

| Tab | Contents |
|---|---|
| Catalog | New Catalog… · Open Catalog… |
| Edit | Undo · Redo · — · Delete Rejected Photos… |
| Photo | Flag Pick/Reject/Unflag · — · Label … · — · Compare · Show in Finder · Copy to Folder… · Import XMP · Write XMP Sidecars · Export Metadata (CSV)… · — · Delete · — · Find People · Re-cluster All Faces · New Smart Album from Filters… |
| View | Toggle Info Panel · Preview · Loupe · People · — · Zoom In · Zoom Out · — · Hide Rejects |

Every major selection action has a **menu path** (with its shortcut shown) so it's discoverable without memorising keys — the menu is the canonical "what can this app do?" surface. Right-click menus and the cull strip are faster paths to the same actions, not the only path.

### Main browse layout

```
menu bar (fill width, MENU_BAR_HEIGHT = 26 px)
row
  sidebar (user-resizable, default SIDEBAR_WIDTH = 220 px, range 140–400 px)
  resize handle (SIDEBAR_HANDLE_WIDTH = 5 px, drag to adjust sidebar width)
  grid (fill)
  [detail panel] (SIDEBAR_WIDTH, optional)
status bar (fill width, fixed height — status text only, no action buttons)
```

Sidebar width is runtime state (user-resizable). `SIDEBAR_WIDTH` is the default and is also used for the detail panel width (which is not resizable).

### Content layout — Grid vs List

The Browse content area renders files in one of two layouts, toggled by a pair of icon buttons (`▦` Grid / `≡` List) at the right of the toolbar, before the Sort control. **List is a sub-mode of Browse, not a separate `ViewMode`** — the sidebar Filters section, search, sort, selection model, detail panel, drag-to-album, and context menus all stay live and identical in both. State lives in `App::grid_layout` (`GridLayout::{Grid, List}`), in-memory, default Grid.

- **Grid** (default) — the thumbnail grid (`view_tile`), `cols` per row computed from tile size and width.
- **List** — one file per line (`view_list_row`), like Capture One's Browser list / Finder list view. Columns, left→right: small thumbnail · **Name** · flag glyph · rating stars · colour dot · Date · Size · Type, then trailing slack (a `Fill` spacer). Row height `LIST_ROW_HEIGHT` (32 px). The hovered row gets a faint fill (derived from the tracked cursor via `tile_index_at`, since the global mouse model has no per-widget hover); a filename clipped by its column width gets a hover tooltip with the full name (same affordance as sidebar labels).
  - A clickable **column-header strip** (`view_list_header`, height `LIST_HEADER_HEIGHT`) sits above the rows. The four real `SortField`s — Name/Date/Size/Type — sort on click; clicking the **active** column toggles direction (`▲`/`▼` shown on it). Flag/Rating/Colour columns are display-only (no matching sort field). This is the same sort state as the toolbar Sort control — one source of truth.
  - **Resizable columns.** Name/Stars/Date/Size/Type are user-resizable; their widths live in `App::list_col` (`ListColWidths`, in-memory, clamped `LIST_COL_MIN..LIST_COL_MAX`). Each carries a right-edge drag handle (thin `BORDER` separator, horizontal-resize cursor). Resize follows the **sidebar-resize pattern**: handle press → `ListColResizeStart` records the start cursor x + width; `MouseMoved` sets the width to `start_w + (cursor.x − start_x)`; `MouseReleased` ends. Trailing slack absorbs the remainder, so a column's right edge tracks the cursor. Thumbnail/flag/colour are fixed glyph columns. A press in the header strip is excluded from grid selection by `in_list_header_band` (so resizing/sorting never clears the photo selection).

**Geometry invariant:** both layouts share the same virtualised scroller and the same hit-test (`tile_index_at`) — clicks, drag-select, right-click, and double-click-to-loupe are *not* re-implemented per layout. With the cull strip and filter panel moved into the sidebar, the content area above the grid is now a **single fixed-height row** (`TOOLBAR_HEIGHT`), so `tile_index_at` subtracts exactly `TOOLBAR_HEIGHT` plus, in List, the `LIST_HEADER_HEIGHT` offset (the header strip shifts rows down). The layout only changes three geometry inputs: `cols()` (1 in List), `row_step()` (`LIST_ROW_HEIGHT` in List), and that `LIST_HEADER_HEIGHT` offset. Keep those in lockstep with the view (→ `architecture.md`, Grid layout & hit-testing).

### Welcome screen

```
container (fill, BG_GRID, padding [20, 24], horizontally centred)
  column (fill height, max-width 960)
    app title + subtitle
    "Recents" section (fill height, scrollable internally)
    action row (Open · New Catalog... · Browse...) [pinned to bottom]
```

Recents takes available vertical space. Actions are always visible — they do not scroll out of view. No vertical centering of the whole column; content is top-anchored and the recents region absorbs resize.

### Search box

The **sidebar search box** (pinned at the top of the sidebar, above the scrollable sections) runs a full-text query over **filename, folder, tags, and descriptive metadata** (title, caption, creator, subjects — folded into the FTS index). A single bareword does **prefix** matching (type-ahead). A query with spaces/quotes is a full **FTS5 expression**: implicit AND between terms, plus `OR` / `NOT`, `"exact phrases"`, and `col:term` column filters (`filename:`, `tags:`, `folder:`). Malformed expressions degrade to a prefix search rather than erroring. Combines with all structured filters.

**Discoverability:** the search input placeholder reads *"Search…"* A hover tooltip reads: *"Supports OR, NOT, \"phrase\", and col:term prefix filters (filename:, tags:, folder:)."* The FTS syntax is also listed in the `?` help panel under Search.

### Filters (sidebar section)

All non-search query controls live in **one collapsible Filters panel** (`view_sidebar_filters`), led by a Class-A *Filters* header (`●` when any filter is active; toggled by `F`, clicking the header band, or the chevron). The panel is **pinned to the sidebar bottom**, not in the scrollable section list (→ *Ordering & grouping*) — the lower of the two stacked sidebar panels (navigation above, filtering below). Collapsed by default. Keeping filters in the sidebar — rather than a band above the grid — means the grid never reflows when filters open and hit-testing stays fixed (→ *Content layout*); pinning them at the *bottom* (rather than leading the section list) keeps the navigation the user just acted on visible and matches the *pick-then-narrow* mental model.

**Layout — regular two-column grid.** Every row is `filter_field(label, controls)`: a fixed-width label column (`FILTER_LABEL_W`, `TEXT_XS`/`FG_DIM`) + a controls block that fills the rest, so labels share one column and controls share one left edge instead of each row finding its own shape. Controls are **one uniform chip family** — `txt_chip` (text toggles/segments) and `glyph_chip` (a glyph with a hover tooltip, for flags/rating/colour), both at one size and padding. Overflowing chip groups `.wrap()`; sub-rows that continue a field (the tag chips+input under *Tags*, the date presets under *To*) use an empty-label `filter_field("", …)` so they stay aligned to the controls column. Do **not** hand-build filter rows with ad-hoc labels/padding — route through `filter_field` + the chip helpers. Top→bottom:

- **Flags** — `✓ ○ ✕` (Pick / Unflagged / Reject), independent toggles forming an OR set: enabling any subset shows files matching *any* enabled flag; empty or all-three both mean *no filter*. Single source of truth for flag filtering — the "Hide Rejects" affordance and `\` are a convenience that sets `{Pick, Unflagged}`.
- **Rating** — the comparator `≥ = ≤` then star counts `1–5` (gold), then `0` = unrated. The comparator combines with a count to form the filter, so "unrated only", "exactly 2", "≤ 1" are all expressible — not just "≥ N". Clicking the active count (or `0`) clears back to Any.
- **Colour** — five colour-dot toggles (Red/Yellow/Green/Blue/Purple); each dot keeps its swatch colour, clicking the active one clears. Independent of stars; also set with keys `6`–`9` or the Loupe swatches, stored as XMP `xmp:Label`. Swatch colours from `styles::color_label_swatch`; shown as a dot on grid tiles and in Loupe.
- **Advanced criteria** — tags (with All/Any match toggle and include/exclude chips), date range (`From`/`To` rows + preset chips), file type, GPS (`Any`/`Yes`/`No`), person, added-within, and camera (person/camera rows appear only when such values exist).
- **Actions** — shown only when filters are active: **Clear**, plus **Save as Smart Album** / **Update Smart Album** (with an inline name input and an "Unsaved" marker when a loaded smart album is dirty).

**Active-filter indicator (grid toolbar):** whenever a filter is narrowing the current collection, an accent **"Filtered ✕"** chip appears at the left of the grid toolbar (`active_chip_style`, tooltip "Filters are narrowing this view — click to clear"); clicking it runs `ClearFilters`. This is the always-on, *near-the-content* signal that the grid is showing a subset — without it, landing in a filtered view (e.g. syncing a folder while a filter is active) silently shows fewer photos than the collection holds, with no visible cause. It complements the sidebar Filters header's `●` marker (which says *a* filter is set; the toolbar chip says *this view is being narrowed right now*).

**Filter model:** every active constraint — flags, rating, colour, search, and advanced criteria — is **ANDed** into a single query. There is one filter state in `App`; all controls read from and write to its fields. There is no separate "cull filter" vs "search filter."

**Smart albums:** **"New Smart Album from Filters…"** (Photo menu) and the **"Save as Smart Album"** button invoke the same action — both expand the Filters section (`SaveAsSmartAlbum` and loading a smart album for edit both `remove` Filters from `collapsed_sections` so the inline name input / criteria are visible) and create or update a smart album from the current filter state. The menu entry is the off-row discoverable path; the section button is the in-context shortcut.

---

## Interaction Patterns

| Pattern | Rule |
|---|---|
| Single-click on recent catalog | Highlight (select), do not open |
| Open recent catalog | Requires explicit "Open" button press |
| Single-click on grid tile | Select only that tile; it becomes the range anchor. (Clicking an already-selected tile keeps the multi-selection so a drag can start.) |
| Cmd+click on grid tile | Toggle that tile in/out of the selection and make it the new anchor. |
| Shift+click on grid tile | Select the range from the anchor to the clicked tile, **replacing** the previous range — so clicking back toward the anchor *shrinks* the selection — while preserving any disjoint Cmd-picked tiles. *(Selection-model internals → `architecture.md`, Grid selection model.)* |
| Double-click on grid tile | Open loupe |
| Enter in tag input | Confirm tag (not bound to loupe) |
| Cmd+= / Cmd+− | Tile size up / down |
| Arrow keys in grid | Move selection (focus follows; grid position retained on loupe exit and folder switch) |
| Shift+Arrow in grid | Extend/shrink the range from the anchor toward the moving end (same model as Shift+click) |
| Arrow keys in loupe | Navigate to prev/next photo |
| Scroll / two-finger trackpad in loupe | Zoom in/out toward the cursor (fit → 8×) |
| Drag in loupe (when zoomed) | Pan; clamped to the image edges |
| Loupe zoom buttons (− / + / 1:1 / Fit / 🔒 / ⛶) | Same zoom state as gestures; **1:1** (or `Z`) toggles actual-pixel zoom, Fit resets to fit-to-window, **🔒** locks zoom+pan across navigation (focus-checking a burst), **⛶** toggles fullscreen. Zoom/pan reset on navigate *unless* locked. *(Why buttons and gestures share one zoom state, and the RAW preview-first decode → `architecture.md`, Loupe image.)* |
| Delete / Backspace in a manual album | Remove selected photos from the album (non-destructive; files untouched) |
| Right-click on sidebar entity | Open context menu |
| Ctrl+Click on sidebar entity | Open context menu (macOS convention) |
| Right-click on grid tile | Open context menu |
| Ctrl+Click on grid tile | Open context menu (macOS convention) |
| Drag tile to album | Immediate drop target highlight; adds on release |
| Rename (from context menu) | Inline input replaces row, pre-filled and **auto-focused** (cursor lands in it — no second click); Enter confirms, Escape cancels |
| Create album / group | `+` menu → inline input appears **auto-focused**. Confirming creates the entity in the sidebar but does **not** navigate to it — the current view (e.g. a folder mid-cull) is left undisturbed; a status line confirms creation. *(Duplicate, by contrast, does select the copy.)* |
| Album delete / folder remove | Context menu item → two-step inline confirm replaces row |
| Smart album save | Name input appears inline in the sidebar Filters section, **auto-focused**, confirmed with Save |
| Smart album "Edit Criteria" | Selects album, expands the sidebar Filters section |
| `.` key (grid) | Repeat last tag — applies most recent tag to current selection |
| `B` key | Add selection to the **target album** (set one via an album's context menu → "Set as Target Album"; marked `◎` in the sidebar). Mirrors Lightroom's quick-collection add for fast keeper-gathering. |
| `?` key | Toggle shortcut help panel |
| `\` key | Toggle hide rejects |
| Sort control (grid toolbar) | `pick_list` dropdown of fields (Name / Date Shot / Size / Type) + a `▲`/`▼` direction toggle button. Not a cycle button — the field set is explicit and visible. |
| Grid / List toggle (toolbar) | `▦` / `≡` icon buttons switch the Browse content layout (thumbnail grid ⇄ compact columnar list). Active layout shown with `active_chip_style`. Pure presentation — no reload; the anchor stays scrolled into view. |
| Thumbnail size (grid toolbar) | A **slider** (`TILE_SIZE_MIN..=MAX`, flanked by small→large `▪ ▰` glyphs, tooltip "Thumbnail size") sets `tile_px` continuously — reads as *size* where a `+`/`−` pair read ambiguously as zoom. `⌘−`/`⌘+` still step it. Grid layout only — hidden in List (fixed row height). |
| Click a List column header | Sort by that field (Name / Date / Size / Type); clicking the already-active column toggles direction. Shares the toolbar Sort state. Flag/Rating/Colour headers are display-only. |
| Drag a List column's right edge | Resize that column (Name / Rating / Date / Size / Type). Width is clamped and held in memory for the session; does not clear the photo selection. |
| Hide Rejects (`\` / View menu) | Convenience toggle between the `{Pick, Unflagged}` flag selection and "show all" — there is no separate hide-rejects state; it's a shortcut into the Filters section's flag set (single source of truth). |
| ⧉ Stack (grid toolbar) | Collapse bursts (shots detected within ~3 s) to one representative tile (the earliest). A burst tile carries a `⧉ N` badge (N = burst size); the badge also shows on burst members when not collapsed. Toggle off to cull within a burst. |

---

## Settings dialog

Triggered by the `⚙` icon (menu bar) or `Cmd+,`. A modal dialog (`BG_MODAL` background, 10 px radius, 24 px padding, 560 px fixed width) with a tabbed header: **General** and **Extensions**.

### Row anatomy

Each setting is one row: label (left, `TEXT_BASE`/`FG`, fills) + control (right, fixed width). Rows are separated by `SPACE_2` vertically. Section groupings within a tab use a `TEXT_SM`/`FG_DIM` section header with `SPACE_4` above it.

### Control types

| Setting type | Control |
|---|---|
| Boolean | Toggle switch |
| Enum / choice | `pick_list` dropdown |
| Numeric with units | Narrow `text_input` (right-aligned) + `TEXT_SM`/`FG_DIM` units label to its right |
| File path | `text_input` (fills) + `ghost_btn_style` "Browse…" button |

### Extension capability preference

When two or more installed extensions share a capability (e.g. `classify`), a chip-selector row appears in Settings → Extensions: label (`TEXT_BASE`) + horizontal chip group (one chip per competing extension, `active_chip_style` for the preferred one). Selecting a chip updates the preference immediately — no Save/Cancel at the row level.

---

## Responsive behaviour

- Max-width 960 on welcome content column.
- Sidebar is user-resizable (140–400 px), default 220 px. Drag the 5 px handle between sidebar and grid.
- Grid fills remaining width; tile count recalculates on scroll event carrying new width.
- Modals are fixed-width (420 px) and centred; window must be wider than modal to display correctly — this is acceptable for a desktop-first app.
- Do not use `align_y(Center)` on full-screen containers when content height may exceed window height. Structure layouts so fill regions absorb resize instead.
