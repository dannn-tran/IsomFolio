module IsomFolio.Core.Storage.Schema

let pragmas = """
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA cache_size=-32000;
PRAGMA temp_store=MEMORY;
PRAGMA mmap_size=268435456;
PRAGMA foreign_keys=ON;
PRAGMA busy_timeout=5000;
"""

let createFiles = """
CREATE TABLE IF NOT EXISTS files (
    id              TEXT PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    filename        TEXT NOT NULL,
    folder          TEXT NOT NULL,
    extension       TEXT NOT NULL,
    size            INTEGER NOT NULL,
    modified_time   INTEGER NOT NULL,
    is_orphaned     INTEGER NOT NULL DEFAULT 0,
    orphaned_at     INTEGER,
    created_at_unix INTEGER NOT NULL DEFAULT 0
);
"""

let createMetadata = """
CREATE TABLE IF NOT EXISTS metadata (
    file_id     TEXT PRIMARY KEY,
    rating      INTEGER,
    label       TEXT,
    title       TEXT,
    description TEXT,
    creator     TEXT,
    subjects    TEXT,
    apple_tags  TEXT,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);
"""

let createTags = """
CREATE TABLE IF NOT EXISTS tags (
    file_id TEXT NOT NULL,
    tag     TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (file_id, tag),
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);
"""

let createTagsIndex = """
CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);
"""

/// Plain (non-content) FTS5 table — no content= link so UPDATE is supported.
/// Triggers keep filename/folder in sync; tags/metadata columns updated explicitly.
let createFileIndex = """
CREATE VIRTUAL TABLE IF NOT EXISTS file_index USING fts5(
    filename,
    tags,
    folder,
    tokenize='unicode61'
);
"""

let createTriggerInsert = """
CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
    INSERT INTO file_index(rowid, filename, tags, folder)
    VALUES (new.rowid, new.filename, '', new.folder);
END;
"""

let createTriggerDelete = """
CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
    DELETE FROM file_index WHERE rowid = old.rowid;
END;
"""

let createTriggerUpdate = """
CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
    UPDATE file_index
    SET filename = new.filename, folder = new.folder
    WHERE rowid = new.rowid;
END;
"""

let createAlbums = """
CREATE TABLE IF NOT EXISTS albums (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL,
    query_json TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0
);
"""

let createAlbumFiles = """
CREATE TABLE IF NOT EXISTS album_files (
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    file_id  TEXT NOT NULL REFERENCES files(id)  ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (album_id, file_id)
);
"""

let createAlbumFilesIndex = """
CREATE INDEX IF NOT EXISTS idx_album_files_album ON album_files(album_id);
"""

/// Migrations — each entry is tried once per open; failures silently ignored (already applied).
/// Runs BEFORE allDdl so it can drop/alter tables that allDdl then recreates.
let migrations = [|
    "ALTER TABLE files ADD COLUMN created_at_unix INTEGER NOT NULL DEFAULT 0"
    // Recreate FTS5 as plain table (old schema used content='files' which blocks UPDATE)
    "DROP TRIGGER IF EXISTS files_ai"
    "DROP TRIGGER IF EXISTS files_ad"
    "DROP TRIGGER IF EXISTS files_au"
    "DROP TABLE IF EXISTS file_index"
|]

/// All DDL statements to run at DB init, in dependency order.
/// Runs AFTER migrations so dropped objects are recreated here.
let allDdl = [|
    createFiles
    createMetadata
    createTags
    createTagsIndex
    createFileIndex
    createTriggerInsert
    createTriggerDelete
    createTriggerUpdate
    createAlbums
    createAlbumFiles
    createAlbumFilesIndex
|]
