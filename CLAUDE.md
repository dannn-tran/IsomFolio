# IsomFolio — Claude Code instructions

## UI design system

**Before making any UI change, read `dev-docs/design-system.md` in full.**

The design system is the single source of truth for all visual and interaction decisions. Key rules to enforce:

- **Icon-only buttons** (single glyph, no text label) → `icon_btn_style`. Never `ghost_btn_style` for a bare icon.
- **No action buttons on entity rows.** No `•••`, no inline ×/✎/+. Context menu only (right-click / Ctrl+Click).
- **Folder rows** → `FOLDER_ITEM_HEIGHT` (28 px). Album rows → `ALBUM_ITEM_HEIGHT` (44 px). Do not normalise them.
- **Context menu trigger** → right-click or Ctrl+Click. Do not add hover-revealed buttons to open it.
- **Spacing** → always use `SPACE_*` constants. Never bare px literals except in one-off compositing layers.
- **Typography** → always use `TEXT_*` constants. Do not create new size tiers.
- **Colour** → use semantic tokens from `styles.rs`. Never hardcode `Color` literals for semantic roles.

When a UI decision is made in a session and is not already in `dev-docs/design-system.md`, update it before finishing.

## Code style

- See `~/.claude/CLAUDE.md` for global coding preferences.
- Rust crate structure: `isomfolio-core` (domain logic, storage, indexing) + `isomfolio-app` (iced UI, state, messages).
- State in `App` struct (`src/app/mod.rs`). Messages in `src/app/types.rs`. Update logic in `src/app/update.rs`. Views in `src/view/`.
- **Keyboard shortcuts** → defined in `src/app/keybinds.rs` as a declarative table. Add new shortcuts there, not in the subscription closure. The help panel (`?` key) auto-generates from the same data.
- **Service layer** → app code talks to `Catalog` (in `isomfolio-core/src/catalog.rs`), never to `db::` or `scanner::` directly.
- **Tag origin** → tags have an `origin` column (manual/ai). Use `upsert_tags` for manual, `add_tags_merge`/`insert_pending_tags` for AI.
- **Addon communication** → use `send_many()` for batch classify (pipelined individual requests). Use `call()`/`call_long()` for single operations (face clustering). Never use `call()` in a loop for batch work.
- Always write tests alongside new features (see global CLAUDE.md).
