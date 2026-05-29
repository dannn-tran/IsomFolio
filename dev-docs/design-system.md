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
| `TEXT_MD` | 12 | Compact body, button labels | `FG` |
| `TEXT_SM` | 11 | Metadata, section headers, chips | `FG_DIM` |
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
| `ALBUM_ITEM_HEIGHT` | 44 | Albums (manually curated — "precious") |
| `FOLDER_ITEM_HEIGHT` | 28 | Folders (file system entries — compact, utilitarian) |

Folder rows are intentionally more compact. Do not normalise them to `ALBUM_ITEM_HEIGHT`.

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
| Grid tile (single) | Open in Loupe · Add to Album ▶ · — · Auto-tag · — · Show in Finder |
| Grid tile (multi-select) | Add to Album ▶ · — · Auto-tag · — · (count label, no loupe) |
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

### Main browse layout

```
row
  sidebar (user-resizable, default SIDEBAR_WIDTH = 220 px, range 140–400 px)
  resize handle (SIDEBAR_HANDLE_WIDTH = 5 px, drag to adjust sidebar width)
  grid (fill)
  [detail panel] (SIDEBAR_WIDTH, optional)
status bar (fill width, fixed height)
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

### Criteria / filter panel

Inline, below search bar, above grid. Expands the grid area rather than overlaying it. Rows: tags, date range, file type toggles, actions.

---

## Interaction Patterns

| Pattern | Rule |
|---|---|
| Single-click on recent catalog | Highlight (select), do not open |
| Open recent catalog | Requires explicit "Open" button press |
| Single-click on grid tile | Select (safe, immediate) |
| Cmd+click on grid tile | Toggle multi-select |
| Shift+click on grid tile | Range select from anchor |
| Double-click on grid tile | Open loupe |
| Enter in tag input | Confirm tag (not bound to loupe) |
| Cmd+= / Cmd+− | Tile size up / down |
| Arrow keys in grid | Move selection |
| Arrow keys in loupe | Navigate to prev/next photo |
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

---

## Responsive behaviour

- Max-width 960 on welcome content column.
- Sidebar is user-resizable (140–400 px), default 220 px. Drag the 5 px handle between sidebar and grid.
- Grid fills remaining width; tile count recalculates on scroll event carrying new width.
- Modals are fixed-width (420 px) and centred; window must be wider than modal to display correctly — this is acceptable for a desktop-first app.
- Do not use `align_y(Center)` on full-screen containers when content height may exceed window height. Structure layouts so fill regions absorb resize instead.
