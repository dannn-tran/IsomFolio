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
- **Interactive hit-target** ≥ 24 px in its smaller dimension (pad small glyph controls to reach it).
- **Meaning is never opacity-only.** A dimmed/greyed state must also differ by another channel (icon, label, position) so it survives low-contrast displays and colour-blindness.

### Discoverability rules

- **Every icon-only / glyph control MUST have a hover tooltip** (`styles::tip`). A bare glyph with no label and no tooltip is a defect.
- **Every action has an off-row, discoverable path.** Right-click / gestures are the *fast* path, never the *only* path: each must also be reachable via a menu entry and/or be documented in the `?` help panel (which lists gestures and right-click menus, not just key bindings).
- **New capability checklist:** before a feature ships, name the path a first-time user finds it through. If the only answer is "they already knew the key" or "they happened to right-click," add a visible/menu/tooltip path first.

---

## Colour Tokens

Defined in `src/view/styles.rs`. Never hardcode raw `Color` literals for semantic roles — use these constants.

| Token | Usage |
|---|---|
| `BG_SIDEBAR` | Sidebar, detail panel background |
| `BG_GRID` | Main content area, welcome screen background |
| `BG_STATUSBAR` | Status bar strip |
| `BG_CRITERIA` | Criteria/filter panel background |
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
| `icon_btn_style` | Icon-only buttons in section headers and toolbars (+ ⚡ etc.). No background — text colour brightens on hover. No box, no padding box. |
| `ghost_btn_style` | Secondary text actions, toggles, chip/filter in off state |
| `active_chip_style` | Primary action (enabled), toggled-on state |
| `danger_btn_style` | Destructive confirm (delete, remove) |
| `quiet_btn_disabled_style` (welcome.rs local) | Primary action when preconditions unmet |

**Rule:** any button that is icon-only (single glyph, no text label) MUST use `icon_btn_style`. `ghost_btn_style` is only for buttons that carry a text label or are inside a content region where a subtle box hover is expected.

Buttons without `on_press` are visually disabled. Do not use `ghost_btn_style` for a primary action when an `active_chip_style` primary button exists nearby.

---

## Component Patterns

### Entity row anatomy

An **entity** is any named, managed object: sidebar folder, sidebar album, grid tile, recent catalog item, **person (face-cluster card)**. The rules:

- The row shows: name, optional read-only status badges (photo count, ⚡ smart indicator, scan spinner, dirty dot).
- The row does **not** embed action buttons at rest, on hover, or on selection. No inline ×, ✎, •••, or similar.
- Context menu (right-click / Ctrl+Click) is the **fast** path to row actions.
- But the context menu is **not the only** path: every row action must *also* be reachable off-row — via a menu entry (e.g. the Photo menu mirrors tile actions; the Catalog menu mirrors folder/album actions for the selected entity) and/or be enumerated in the `?` help panel's "Right-click menus" section. (See *Discoverability rules* in Philosophy.)

**Why no row buttons:** embedding buttons conflates display with action; context menus scale to any number of actions without visual weight; hover-revealed overflow buttons add clutter and a small hit area. **Why also off-row:** a right-click-only action with no cue is undiscoverable — the row gives no hint the menu exists. Quiet rows are kept; the action's *existence* is surfaced elsewhere.

Do not add action buttons to entity rows. Do not make an action reachable *only* by right-click.

### Row heights

Two row height constants exist to express the hierarchy between containers and items:

| Constant | px | Used for |
|---|---|---|
| `ALBUM_ITEM_HEIGHT` | 32 | Albums (manually curated — slightly taller than folders) |
| `FOLDER_ITEM_HEIGHT` | 28 | Folders (file system entries — compact, utilitarian) |

Folder rows are intentionally more compact. Do not normalise them to `ALBUM_ITEM_HEIGHT`.

### Folder tree

Folders render as a navigable **tree**, not a flat list, showing the deepest folders the user actually has photos under (pass-through ancestors are collapsed away — never `/`, `/Users`, etc.). *(How the tree is built/collapsed and where expansion/scan-depth state live → `architecture.md`, UI rendering.)*

- **Expand/collapse** → a leading chevron (`▸` collapsed, `▾` expanded), `icon_btn_style`, in a fixed-width slot so all labels align regardless of whether a row has children. The chevron is a separate control and does not change selection.
- **Indentation** → each depth level adds `SPACE_3` of leading space. The truncation budget shrinks with depth so deep labels still clip cleanly with a tooltip.
- **Count** → the photo count includes the folder *and* all descendants, not just direct children.
- **Selection** → clicking the label selects the folder and loads its photos recursively.
- **Dirty dot** → an accent `●` after the folder name means the watcher saw structural changes on disk (files added / removed / renamed) that have not been applied. The catalog is never mutated silently — the user applies the changes by syncing the folder (`Cmd+R` or context menu), which clears the dot. (Pure content edits to an already-tracked file are not structural: they just refresh that file's thumbnail, no dot.)

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
- Destructive item text: `ERR` (no background change — colour alone signals danger)

**Dismiss:** click outside the menu, press Escape, or select any item.

**Context menu contents by entity type:**

| Entity | Menu items |
|---|---|
| Folder | Sync Folder · (Remove Missing Files…, when orphans present) · — · Remove from Library… |
| Manual album | Rename · Duplicate · Set/Clear Target Album · — · Delete… |
| Smart album | Rename · Duplicate · Edit Criteria · — · Delete… |
| Grid tile (single) | Open in Loupe · Add to Album ▶ · — · Import XMP metadata · (Import Apple Finder tags, macOS) · Show in Finder / Locate… · Copy to Folder… · Move to Folder… |
| Grid tile (multi-select) | Add to Album ▶ · — · Import XMP metadata · (Import Apple Finder tags, macOS) · Show in Finder · Copy to Folder… · Move to Folder… |
| Person (face card) | Rename · Merge into ▶ |
| Recent catalog item | Open · Remove from Recents |

Each of these also has an off-row path (Photo / Catalog menus) or is listed in the help panel — see *Entity row anatomy*.

Ellipsis (…) in a menu item label signals the action has a secondary step (rename → inline input; delete → inline confirm; add to album → submenu). Items without ellipsis execute immediately.

**Confirmation from context menu:** destructive items (Remove from Library…, Delete…) close the context menu and replace the entity row with `confirm_action_row` inline. The confirm pattern itself is unchanged — only the trigger mechanism moves from an embedded button to the menu.

**Submenus (Add to Album ▶):** render as a second context menu panel to the right of the parent item. List all manual albums by name. Selecting one adds the dragged/selected files immediately (safe, no confirm). If no albums exist, show a disabled "No albums yet" item.

### Selection states

Use a translucent `ACCENT` overlay (α ≈ 0.22–0.28) for selected items. Do not change text colour on selection unless contrast demands it. Use a 3 px `ACCENT` ring for grid tiles. Use a rounded background fill for list items (sidebar albums, recent catalogs).

### Reject display (dim, don't remove)

A rejected grid tile is **dimmed in place** (dark scrim, α ≈ 0.55) rather than removed — the grid keeps its continuity during a cull and a reject stays one click from being un-rejected, instead of vanishing and reflowing the layout. Exceptions: a *selected* or *being-dragged* reject is shown normally (you're acting on it), and when the view is filtered to **rejects only** they're shown normally (you're reviewing them deliberately). "Hide Rejects" / the flag filter still *removes* rejects entirely when the user explicitly wants them gone — dimming is the default in-place state, hiding is the opt-in.

### Delete is virtual (Deleted folder)

**Delete never touches the file on disk.** "Delete" (the `Del`/`Backspace` key, the Photo menu, or "Delete Rejected Photos") sets a virtual `is_deleted` flag in the catalog: the photo drops out of every normal view and collects in a virtual **Deleted** sidebar entry (shown only when non-empty, with a count). **Restore** (right-click in the Deleted view) clears the flag — instant and lossless, because the row never left the catalog (ratings/tags intact). There is no on-disk trash folder and no file move. (Inside a manual album, `Del` instead unlinks from the album.) Implementation invariant — the flag survives re-sync — is in `architecture.md`.

**Permanent purge** is the one exception that touches disk: "Delete Permanently…" (Deleted-view context menu, on a selection) or "Empty Deleted…" (status bar) deletes the actual files and removes the rows. It is irreversible, so it uses the inline `ERR` confirm (Cancel + Delete) before acting.

### Confirmation pattern

Two-step for destructive ops: first trigger (context menu item) → inline confirm row appears on the entity (prompt in `ERR`, Cancel + Confirm buttons). `confirm_action_row()` helper in styles.rs.

Single-step for safe ops: primary button directly triggers action.

The trigger for a destructive op is always the context menu item, never a persistent inline button.

### Disabled primary button

Show the button at reduced opacity (`FG_MUTED` text, α 0.04 background) without `on_press`. Never hide a primary button — always show its position so the user understands what is needed to unlock it.

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

Each tag row has: leaf name, file count, "+" (apply to current file), "Rename", "Delete" (`ERR`). Rename and delete have inline confirm states.

### Shortcut help panel

Modal overlay (340 px), triggered by `?` key. Keyboard bindings grouped by category (Navigation, View, Culling, Tagging). Each row: key combo in `ACCENT` (100 px column) + label in `FG`. Dismissed by Escape or ✕ button.

Bindings defined declaratively in `keybinds.rs` — the help panel iterates the same data. Adding a shortcut = one line in the binding table.

### Error display

Inline, near the cause. Use `ERR` colour. Short copy. No modal for validation errors.

### Non-happy states

Every content area must define what it shows when it isn't full of content. Named patterns:

| State | Pattern |
|---|---|
| **Empty — no library** | Onboarding call-to-action centred in the grid: heading (`TEXT_MD`/`FG`) + one line of guidance (`TEXT_SM`/`FG_DIM`) + a primary button (`active_chip_style`). Never a bare "nothing here". e.g. "No photos yet — Add a folder to start your catalog" + **Add Folder…**. |
| **Empty — filtered/album** | Quiet single line: "No photos in this view" (`TEXT_BASE`/`FG_DIM`). The user created this state, so no CTA. |
| **Loading — thumbnails** | Tile placeholder (`BG_TILE_LOADING`) per tile until ready; aggregate progress in the task panel. Never block the grid. |
| **Capability absent** | When a feature needs an uninstalled extension (e.g. People with no engine), the view explains it and links to where to enable it (Settings → Extensions) rather than showing an empty or broken control. |

The distinction matters: an *empty library* is a dead end the app must help the user out of (CTA); an *empty filter result* is an expected, user-created state (quiet line, no nag).

### Modal dialogs

Use `stack` overlay: base layer + semi-opaque scrim (`Color { r:0, g:0, b:0, a:0.55 }`) + centred modal card. Modal card: `BG_MODAL` background, 10 px radius, 24 px padding, fixed width (≈ 420 px). Reserve modals for focused multi-field task flows (e.g. New Catalog). Do not use modals for simple toggles or confirmations.

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
| Photo | Flag Pick/Reject/Unflag · — · Label Red/Yellow/Green/Blue/Purple/Remove · — · Compare · Copy/Move to Folder… · Import XMP · — · Delete · — · Find People · New Smart Album from Filters… |
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

### Welcome screen

```
container (fill, BG_GRID, padding [20, 24], horizontally centred)
  column (fill height, max-width 960)
    app title + subtitle
    "Recents" section (fill height, scrollable internally)
    action row (Open · New Catalog... · Browse...) [pinned to bottom]
```

Recents takes available vertical space. Actions are always visible — they do not scroll out of view. No vertical centering of the whole column; content is top-anchored and the recents region absorbs resize.

### Cull strip (always visible)

A **single dense glyph row** (fixed height `CULL_STRIP_HEIGHT`, ≈ one row) sits directly under the toolbar, always visible so the three cull axes are one click away mid-cull without stealing grid rows. Deliberately *not* stacked, labelled-chip rows (cf. Lightroom's one-row filter bar / Photo Mechanic's icon strip) — glyphs, not words. Layout, left→right, separated by faint `│` dividers:

- **Flags** — `✓ ○ ✕` (Pick / Unflagged / Reject), independent toggles forming an OR set: enabling any subset shows files matching *any* enabled flag; empty or all-three both mean *no filter*. Single source of truth for flag filtering — the toolbar "Hide Rejects" chip and `\` are a convenience that sets `{Pick, Unflagged}`.
- **Rating** — a gold `★` marker, the comparator `≥ = ≤`, star counts `1–5`, and `0` = unrated. The comparator combines with a count to form the filter, so "unrated only", "exactly 2", "≤ 1" are all expressible — not just "≥ N". Clicking the active count (or `0`) clears back to Any.
- **Colour** — five colour-dot toggles (Red/Yellow/Green/Blue/Purple); each dot keeps its swatch colour, clicking the active one clears. A second cull axis independent of stars; also set with keys `6`–`9` or the Loupe swatches, stored as XMP `xmp:Label`. Swatch colours from `styles::color_label_swatch`; shown as a dot on grid tiles and in Loupe.

The strip is a **fixed-height single row** — this is a hard requirement, not just a style choice: a variable-height band above the grid would break tile hit-testing (→ `architecture.md`, Grid layout & hit-testing).

### Criteria / filter panel

Inline, below the cull strip, above grid; toggled by `F` / the "Filters" button. Holds the *advanced* (non-cull) criteria only: tags, date range + presets, file type, location, person, camera, added-within, and the Clear / Save-as-Smart-Album actions. Expands the grid area rather than overlaying it.

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
| Rename (from context menu) | Inline input replaces row, pre-filled; Enter confirms, Escape cancels |
| Album delete / folder remove | Context menu item → two-step inline confirm replaces row |
| Smart album save | Name input appears inline in criteria panel, confirmed with Save |
| Smart album "Edit Criteria" | Selects album, opens criteria panel |
| `.` key (grid) | Repeat last tag — applies most recent tag to current selection |
| `B` key | Add selection to the **target album** (set one via an album's context menu → "Set as Target Album"; marked `◎` in the sidebar). Mirrors Lightroom's quick-collection add for fast keeper-gathering. |
| `?` key | Toggle shortcut help panel |
| `\` key | Toggle hide rejects |
| Sort control (grid toolbar) | `pick_list` dropdown of fields (Name / Date Shot / Size / Type) + a `▲`/`▼` direction toggle button. Not a cycle button — the field set is explicit and visible. |
| Hide Rejects (grid toolbar) / `\` | Convenience toggle between the `{Pick, Unflagged}` flag selection and "show all" — there is no separate hide-rejects state; it's a shortcut into the cull strip's flag set (single source of truth). |
| ⧉ Stack (grid toolbar) | Collapse bursts (shots detected within ~3 s) to one representative tile (the earliest). A burst tile carries a `⧉ N` badge (N = burst size); the badge also shows on burst members when not collapsed. Toggle off to cull within a burst. |

---

## Responsive behaviour

- Max-width 960 on welcome content column.
- Sidebar is user-resizable (140–400 px), default 220 px. Drag the 5 px handle between sidebar and grid.
- Grid fills remaining width; tile count recalculates on scroll event carrying new width.
- Modals are fixed-width (420 px) and centred; window must be wider than modal to display correctly — this is acceptable for a desktop-first app.
- Do not use `align_y(Center)` on full-screen containers when content height may exceed window height. Structure layouts so fill regions absorb resize instead.
