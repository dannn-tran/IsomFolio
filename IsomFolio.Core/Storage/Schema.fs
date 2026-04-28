module IsomFolio.Storage.Schema

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
    id            TEXT PRIMARY KEY,
    path          TEXT NOT NULL UNIQUE,
    filename      TEXT NOT NULL,
    folder        TEXT NOT NULL,
    extension     TEXT NOT NULL,
    size          INTEGER NOT NULL,
    modified_time INTEGER NOT NULL,
    is_orphaned   INTEGER NOT NULL DEFAULT 0,
    orphaned_at   INTEGER
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

let createFileIndex = """
CREATE VIRTUAL TABLE IF NOT EXISTS file_index USING fts5(
    filename,
    tags,
    folder,
    content='files',
    content_rowid='rowid',
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
    INSERT INTO file_index(file_index, rowid, filename, tags, folder)
    VALUES ('delete', old.rowid, old.filename, '', old.folder);
END;
"""

let createTriggerUpdate = """
CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
    INSERT INTO file_index(file_index, rowid, filename, tags, folder)
    VALUES ('delete', old.rowid, old.filename, '', old.folder);
    INSERT INTO file_index(rowid, filename, tags, folder)
    VALUES (new.rowid, new.filename, '', new.folder);
END;
"""

let ftsRebuild = "INSERT INTO file_index(file_index) VALUES ('rebuild');"

/// All DDL statements to run at DB init, in dependency order
let allDdl = [|
    createFiles
    createTags
    createTagsIndex
    createFileIndex
    createTriggerInsert
    createTriggerDelete
    createTriggerUpdate
|]
