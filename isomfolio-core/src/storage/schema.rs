pub const PRAGMAS: &[&str] = &[
    "PRAGMA journal_mode=WAL",
    "PRAGMA synchronous=NORMAL",
    "PRAGMA cache_size=-32000",
    "PRAGMA temp_store=MEMORY",
    "PRAGMA mmap_size=268435456",
    "PRAGMA foreign_keys=ON",
    "PRAGMA busy_timeout=5000",
];

pub const CREATE_FILES: &str = "
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
    created_at_unix INTEGER NOT NULL DEFAULT 0,
    flag            INTEGER NOT NULL DEFAULT 0,
    exif_date_unix  INTEGER,
    gps_lat         REAL,
    gps_lon         REAL,
    burst_id        TEXT
);
";

pub const CREATE_METADATA: &str = "
CREATE TABLE IF NOT EXISTS metadata (
    file_id         TEXT PRIMARY KEY,
    rating          INTEGER,
    label           TEXT,
    title           TEXT,
    description     TEXT,
    creator         TEXT,
    subjects        TEXT,
    apple_tags      TEXT,
    camera_make     TEXT,
    camera_model    TEXT,
    lens_model      TEXT,
    focal_length_mm REAL,
    aperture        REAL,
    shutter_speed   TEXT,
    iso             INTEGER,
    flash           INTEGER,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);
";

pub const CREATE_TAGS: &str = "
CREATE TABLE IF NOT EXISTS tags (
    file_id TEXT NOT NULL,
    tag     TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (file_id, tag),
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);
";

pub const CREATE_TAGS_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);";

pub const CREATE_FILE_INDEX: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS file_index USING fts5(
    filename,
    tags,
    folder,
    tokenize='unicode61'
);
";

pub const CREATE_TRIGGER_INSERT: &str = "
CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
    INSERT INTO file_index(rowid, filename, tags, folder)
    VALUES (new.rowid, new.filename, '', new.folder);
END;
";

pub const CREATE_TRIGGER_DELETE: &str = "
CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
    DELETE FROM file_index WHERE rowid = old.rowid;
END;
";

pub const CREATE_TRIGGER_UPDATE: &str = "
CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
    UPDATE file_index
    SET filename = new.filename, folder = new.folder
    WHERE rowid = new.rowid;
END;
";

pub const CREATE_ALBUMS: &str = "
CREATE TABLE IF NOT EXISTS albums (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL,
    query_json TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0
);
";

pub const CREATE_ALBUM_FILES: &str = "
CREATE TABLE IF NOT EXISTS album_files (
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    file_id  TEXT NOT NULL REFERENCES files(id)  ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (album_id, file_id)
);
";

pub const CREATE_ALBUM_FILES_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_album_files_album ON album_files(album_id);";

/// Run once per DB open; errors silently ignored (already applied).
pub const MIGRATIONS: &[&str] = &[
    "ALTER TABLE files ADD COLUMN created_at_unix INTEGER NOT NULL DEFAULT 0",
    "DROP TRIGGER IF EXISTS files_ai",
    "DROP TRIGGER IF EXISTS files_ad",
    "DROP TRIGGER IF EXISTS files_au",
    "DROP TABLE IF EXISTS file_index",
    "ALTER TABLE files ADD COLUMN flag INTEGER NOT NULL DEFAULT 0",
    "ALTER TABLE files ADD COLUMN exif_date_unix INTEGER",
    "ALTER TABLE files ADD COLUMN gps_lat REAL",
    "ALTER TABLE files ADD COLUMN gps_lon REAL",
    "ALTER TABLE files ADD COLUMN burst_id TEXT",
    "ALTER TABLE metadata ADD COLUMN camera_make TEXT",
    "ALTER TABLE metadata ADD COLUMN camera_model TEXT",
    "ALTER TABLE metadata ADD COLUMN lens_model TEXT",
    "ALTER TABLE metadata ADD COLUMN focal_length_mm REAL",
    "ALTER TABLE metadata ADD COLUMN aperture REAL",
    "ALTER TABLE metadata ADD COLUMN shutter_speed TEXT",
    "ALTER TABLE metadata ADD COLUMN iso INTEGER",
    "ALTER TABLE metadata ADD COLUMN flash INTEGER",
];

pub const ALL_DDL: &[&str] = &[
    CREATE_FILES,
    CREATE_METADATA,
    CREATE_TAGS,
    CREATE_TAGS_INDEX,
    CREATE_FILE_INDEX,
    CREATE_TRIGGER_INSERT,
    CREATE_TRIGGER_DELETE,
    CREATE_TRIGGER_UPDATE,
    CREATE_ALBUMS,
    CREATE_ALBUM_FILES,
    CREATE_ALBUM_FILES_INDEX,
];
