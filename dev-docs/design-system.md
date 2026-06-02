# IsomFolio Design System

This document is the single source of truth for visual and interaction decisions across the app. Absorbs the welcome-ui-spec. Apply these rules consistently when building new screens or modifying existing ones.

---

## Philosophy

- **Compact and information-dense.** Minimise chrome. Every pixel should carry content or breathing room, not decoration.
- **Quiet by default.** Use spacing, typography weight, and opacity to express hierarchy. Reserve borders, backgrounds, and colour for meaning (selection, error, active state), not layout structure.
- **Progressive disclosure.** Show the simplest view first. Expand only when the user asks (criteria panel, detail panel, modals).
- **Reversible and confirmed.** Destructive or irreversible actions require explicit confirmation. Single-click navigation and selection are safe and immediate.

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

An **entity** is any named, managed object: sidebar folder, sidebar album, grid tile, recent catalog item. The rule:

- The row shows: name, optional read-only status badges (photo count, ⚡ smart indicator, scan spinner).
- The row does **not** embed action buttons at rest, on hover, or on selection. No inline ×, ✎, •••, or similar.
- All actions are accessed exclusively via context menu (right-click or Ctrl+Click).

**Why:** embedding buttons conflates display with action. Context menus scale to any number of actions without adding visual weight, and provide a single consistent interaction surface across all entity types. Hover-revealed overflow buttons add clutter and require the user to target a small hit area.

Do not add action buttons to entity rows. The context menu is always the right place.

### Row heights

Two row height constants exist to express the hierarchy between containers and items:

| Constant | px | Used for |
|---|---|---|
| `ALBUM_ITEM_HEIGHT` | 32 | Albums (manually curated — slightly taller than folders) |
| `FOLDER_ITEM_HEIGHT` | 28 | Folders (file system entries — compact, utilitarian) |

Folder rows are intentionally more compact. Do not normalise them to `ALBUM_ITEM_HEIGHT`.

### Folder tree

Folders render as a navigable **tree**, not a flat list. The tree is built by `folder_tree::build_tree` from the distinct indexed folder paths (`get_folder_counts`); pure pass-through ancestors (no own photos, a single child) are collapsed so the displayed roots are the deepest folders the user actually has photos under — never `/`, `/Users`, etc.

- **Expand/collapse** → a leading chevron (`▸` collapsed, `▾` expanded), `icon_btn_style`, in a fixed `CHEVRON_W` (16 px) slot. Folders with no children get an equal-width `Space` so all labels align. Toggling fires `ToggleFolderExpanded(path)`; expansion state lives in `App.expanded_folders` (not persisted).
- **Indentation** → each depth level adds `SPACE_3` of leading space. The truncation budget shrinks with depth so deep labels still clip cleanly with a tooltip.
- **Count** → shows `total_count` (photos in the folder *and* all descendants), not just direct children.
- **Selection** → clicking the label selects the folder (`SidebarItem::Folder`) and loads its photos recursively. The chevron is a separate button and does not change selection.
- **Scan depth** → whether subfolders are indexed is chosen once when the folder is added (the "Include subfolders" checkbox in the add-folder dialog) and stored per root in the `library_roots` table. Re-sync honours the stored choice; unknown paths default to recursive.
- **Dirty dot** → an accent `●` after the folder name means the watcher saw structural changes on disk (files added / removed / renamed) that have not been applied. The catalog is never mutated silently — the user applies the changes by syncing the folder (`Cmd+R` or context menu), which clears the dot. (Pure content edits to an already-tracked file are not structural: they just refresh that file's thumbnail, no dot.)

### Context menu

Implemented as a `stack` overlay anchored to the cursor position. No scrim — context menus are non-blocking.

**Trigger:** right-click or Ctrl+Click anywhere on a sidebar entity or grid tile. There is no overflow button. Ctrl+Click is treated as an alias for right-click in `MousePressed` by delegating to `MouseRightClicked` when `self.modifiers.control()` is set.

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
| Folder | Rescan · — · Remove from Library… |
| Manual album | Rename · Duplicate · — · Delete… |
| Smart album | Rename · Duplicate · Edit Criteria · — · Delete… |
| Grid tile (single) | Open in Loupe · Add to Album ▶ · — · Import XMP metadata · (Import Apple Finder tags, macOS) · Show in Finder / Locate… · Copy to Folder… · Move to Folder… |
| Grid tile (multi-select) | Add to Album ▶ · — · Import XMP metadata · (Import Apple Finder tags, macOS) · Show in Finder · Copy to Folder… · Move to Folder… |
| Recent catalog item | Open · Remove from Recents |

Ellipsis (…) in a menu item label signals the action has a secondary step (rename → inline input; delete → inline confirm; add to album → submenu). Items without ellipsis execute immediately.

**Confirmation from context menu:** destructive items (Remove from Library…, Delete…) close the context menu and replace the entity row with `confirm_action_row` inline. The confirm pattern itself is unchanged — only the trigger mechanism moves from an embedded button to the menu.

**Submenus (Add to Album ▶):** render as a second context menu panel to the right of the parent item. List all manual albums by name. Selecting one adds the dragged/selected files immediately (safe, no confirm). If no albums exist, show a disabled "No albums yet" item.

### Selection states

Use a translucent `ACCENT` overlay (α ≈ 0.22–0.28) for selected items. Do not change text colour on selection unless contrast demands it. Use a 3 px `ACCENT` ring for grid tiles. Use a rounded background fill for list items (sidebar albums, recent catalogs).

### Confirmation pattern

Two-step for destructive ops: first trigger (context menu item) → inline confirm row appears on the entity (prompt in `ERR`, Cancel + Confirm buttons). `confirm_action_row()` helper in styles.rs.

Single-step for safe ops: primary button directly triggers action.

The trigger for a destructive op is always the context menu item, never a persistent inline button.

### Disabled primary button

Show the button at reduced opacity (`FG_MUTED` text, α 0.04 background) without `on_press`. Never hide a primary button — always show its position so the user understands what is needed to unlock it.

### Tag section (detail panel)

The detail panel's tag section has three parts, in order:

1. **Confirmed tags** — each as a chip with `render_tag_name` (hierarchy dimming) + remove button. AI-origin tags show a muted badge: "AI 87%" (confidence percentage when available, "AI" alone when not).
2. **Autocomplete suggestions** — appear when the tag input has text. Ranked: prefix match first, then substring, with leaf-segment-aware matching. Max 5 chips.
3. **Recent tags row** — last 8 tags used this session, filtered to exclude tags already on the file. One-click apply. Label "Recent" in `FG_DIM`.
4. **Tag input** — `text_input` with `on_submit` → `AddDetailTag`.
5. **Pending tags (suggested)** — only shown when `pending_tags` is non-empty. Header row: "Suggested" label + "Accept All" / "Reject All" buttons. Each pending tag shows: tag name in `FG_DIM`, confidence percentage in `FG_MUTED`, ✓ (accept, `ACCENT`) and ✕ (reject, `ERR`) buttons. Background: subtle border + α 0.03 white fill to distinguish from confirmed tags.

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

### Modal dialogs

Use `stack` overlay: base layer + semi-opaque scrim (`Color { r:0, g:0, b:0, a:0.55 }`) + centred modal card. Modal card: `BG_MODAL` background, 10 px radius, 24 px padding, fixed width (≈ 420 px). Reserve modals for focused multi-field task flows (e.g. New Catalog). Do not use modals for simple toggles or confirmations.

---

## Layout Patterns

### Menu bar

Custom horizontal bar (height 26 px, `BG_STATUSBAR` background). Left side: content-operation menus (`Catalog`, `Edit`, `View`). Right side: persistent icon-only buttons (`?` → shortcut help, `⚙` → Settings).

**Rule:** Menu tabs are for catalog/content operations. App-level config (Settings) lives on the gear icon button, not in a menu tab. There is no Help tab — keyboard shortcuts are accessed via `?` icon or the `?` key.

| Tab | Contents |
|---|---|
| Catalog | New Catalog… · Open Catalog… |
| Edit | Undo · Redo · — · Move Rejects to Trash… |
| View | Toggle Info Panel · Preview · Loupe · People · — · Zoom In · Zoom Out · — · Hide Rejects |

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

Sidebar width is stored in `App::sidebar_width` (runtime state). The `SIDEBAR_WIDTH` constant is the default and is also used for the detail panel width (which is not resizable).

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

A fixed-height (`CULL_STRIP_HEIGHT`) strip sits directly under the toolbar and is **always visible** — it holds the two primary cull axes so they're one click away mid-cull, never hidden behind a toggle:

- **Flag** — three independent toggle chips (Picks / Unflagged / Rejects). They form an OR set: enabling any subset shows files matching *any* enabled flag. Empty (none) or full (all three) both mean *no filter*. This is the single source of truth for flag filtering; the toolbar "Hide Rejects" chip and the `\` key are a convenience that toggles the strip to the `{Picks, Unflagged}` selection.
- **Stars** — `Any · Unrated · ≥ · = · ≤ · 1–5`. The comparator (`≥/=/≤`) combines with a star-count chip to form the active filter, so "unrated only", "exactly 2", "≤ 1" are all expressible — not just "≥ N".
- **Colour** — `Any` + five colour-dot chips (Red/Yellow/Green/Blue/Purple). Colour labels are a second cull axis independent of stars, set with keys `6`–`9` (Red/Yellow/Green/Blue; press again to clear) or the Loupe swatches, and stored as XMP `xmp:Label`. Swatch colours come from `styles::color_label_swatch`; shown as a dot on grid tiles and in Loupe.

Because the strip is fixed-height, grid hit-testing adds `CULL_STRIP_HEIGHT` to its vertical offset.

### Criteria / filter panel

Inline, below the cull strip, above grid; toggled by `F` / the "Filters" button. Holds the *advanced* (non-cull) criteria only: tags, date range + presets, file type, location, person, camera, added-within, and the Clear / Save-as-Smart-Album actions. Expands the grid area rather than overlaying it.

---

## Interaction Patterns

| Pattern | Rule |
|---|---|
| Single-click on recent catalog | Highlight (select), do not open |
| Open recent catalog | Requires explicit "Open" button press |
| Single-click on grid tile | Select only that tile; it becomes the anchor. (Clicking an already-selected tile keeps the multi-selection so a drag can start.) |
| Cmd+click on grid tile | Toggle that tile and make it the new anchor. The resulting selection is snapshotted as the range *base*. |
| Shift+click on grid tile | Select `base ∪ [anchor..=clicked]`, **replacing** the previous range — so clicking back toward the anchor *shrinks* the selection. Anchor stays fixed; the clicked tile is the moving end. Disjoint Cmd-picks (the base) are preserved. |
| Double-click on grid tile | Open loupe |
| Enter in tag input | Confirm tag (not bound to loupe) |
| Cmd+= / Cmd+− | Tile size up / down |
| Arrow keys in grid | Move selection (focus follows; grid position retained on loupe exit and folder switch) |
| Shift+Arrow in grid | Extend/shrink the range from the anchor toward the moving end (same `base ∪ [anchor..=lead]` model as Shift+click) |
| Arrow keys in loupe | Navigate to prev/next photo |
| Scroll / two-finger trackpad in loupe | Zoom in/out toward the cursor (fit → 8×) |
| Drag in loupe (when zoomed) | Pan; clamped to the image edges |
| Loupe zoom buttons (− / + / 1:1 / Fit / ⛶) | Same zoom state as gestures; **1:1** (or `Z`) toggles actual-pixel zoom (computed from widget-reported viewport+native size), Fit resets to fit-to-window, **⛶** toggles fullscreen. Zoom/pan reset on navigate. Custom `LoupeImage` widget (app-owned scale+pan) — the built-in `image::Viewer` keeps zoom internal and can't be button-driven. |
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
| `?` key | Toggle shortcut help panel |
| `\` key | Toggle hide rejects |
| Sort control (grid toolbar) | `pick_list` dropdown of fields (Name / Date Shot / Size / Type) + a `▲`/`▼` direction toggle button. Not a cycle button — the field set is explicit and visible. |
| Hide Rejects (grid toolbar) | Always-visible toggle chip (`active_chip_style` when on). Mirrors the `\` key and the filter-panel toggle — same `hide_rejects` state. |

---

## Responsive behaviour

- Max-width 960 on welcome content column.
- Sidebar is user-resizable (140–400 px), default 220 px. Drag the 5 px handle between sidebar and grid.
- Grid fills remaining width; tile count recalculates on scroll event carrying new width.
- Modals are fixed-width (420 px) and centred; window must be wider than modal to display correctly — this is acceptable for a desktop-first app.
- Do not use `align_y(Center)` on full-screen containers when content height may exceed window height. Structure layouts so fill regions absorb resize instead.
