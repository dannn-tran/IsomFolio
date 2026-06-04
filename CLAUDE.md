# IsomFolio — Claude Code instructions

## UI design system

**Before making any UI change, read `dev-docs/design-system.md` in full.**

The design system is the single source of truth for all visual and interaction decisions. Key rules to enforce:

- **Icon-only buttons** (single glyph, no text label) → `icon_btn_style`. Never `ghost_btn_style` for a bare icon.
- **No action buttons on entity rows.** No `•••`, no inline ×/✎/+. Context menu only (right-click / Ctrl+Click).
- **Folder rows** → `FOLDER_ITEM_HEIGHT` (28 px). Album rows → `ALBUM_ITEM_HEIGHT` (32 px). Do not normalise them.
- **Context menu trigger** → right-click or Ctrl+Click. Do not add hover-revealed buttons to open it.
- **Spacing** → always use `SPACE_*` constants. Never bare px literals except in one-off compositing layers.
- **Typography** → always use `TEXT_*` constants. Do not create new size tiers.
- **Colour** → use semantic tokens from `styles.rs`. Never hardcode `Color` literals for semantic roles.

When a UI decision is made in a session and is not already in `dev-docs/design-system.md`, update it before finishing.

## Architecture

**Before making any structural change to sync, extensions, tags, or the state model, read `dev-docs/architecture.md`.**

Key invariants (full rationale in the doc):

- **Service layer** → app code talks to `Catalog` (`isomfolio-core/src/catalog.rs`), never to `db::` or `scanner::` directly.
- **Sync model** → catalog is source of truth after first index; external metadata never overwrites user edits on re-sync.
- **Tags** → no origin column, no confidence column (both removed before first release). Use `upsert_tags` for full-replace edits; `add_tags_merge` for additive single-file writes; `add_tag_to_files_bulk` for multi-file tag adds.
- **Extension protocol** → stdout = IEP protocol only; stderr = structured JSON diagnostics. Never mix.
- **Extension batch calls** → use `send_many()` for batch work; `call()`/`call_long()` for single ops. Never use `call()` in a loop.

## Code style

- See `~/.claude/CLAUDE.md` for global coding preferences.
- Rust crate structure: `isomfolio-core` (domain logic, storage, indexing) + `isomfolio-app` (iced UI, state, messages).
- State in `App` struct (`src/app/mod.rs`). Messages in `src/app/types.rs`. Update logic in `src/app/update.rs`. Views in `src/view/`.
- **Keyboard shortcuts** → defined in `src/app/keybinds.rs` as a declarative table. Add new shortcuts there, not in the subscription closure. The help panel (`?` key) auto-generates from the same data.
- Always write tests alongside new features (see global CLAUDE.md).
