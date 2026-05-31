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
    file_id     TEXT NOT NULL,
    tag         TEXT NOT NULL COLLATE NOCASE,
    confidence  REAL,
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

pub const CREATE_FACE_CLUSTERS: &str = "
CREATE TABLE IF NOT EXISTS face_clusters (
    cluster_id  TEXT NOT NULL,
    file_id     TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    bbox_x      REAL NOT NULL,
    bbox_y      REAL NOT NULL,
    bbox_w      REAL NOT NULL,
    bbox_h      REAL NOT NULL,
    PRIMARY KEY (cluster_id, file_id, bbox_x, bbox_y)
);
";

pub const CREATE_FACE_CLUSTER_NAMES: &str = "
CREATE TABLE IF NOT EXISTS face_cluster_names (
    cluster_id  TEXT PRIMARY KEY,
    name        TEXT NOT NULL
);
";

pub const CREATE_FACE_CLUSTER_IDX: &str =
    "CREATE INDEX IF NOT EXISTS idx_fc_cluster ON face_clusters(cluster_id);";

pub const CREATE_FACE_EMBEDDINGS: &str = "
CREATE TABLE IF NOT EXISTS face_embeddings (
    file_id     TEXT NOT NULL,
    mtime       INTEGER NOT NULL,
    bbox_x      REAL NOT NULL,
    bbox_y      REAL NOT NULL,
    bbox_w      REAL NOT NULL,
    bbox_h      REAL NOT NULL,
    vec         BLOB NOT NULL,
    PRIMARY KEY (file_id, bbox_x, bbox_y)
);
";

pub const CREATE_FACE_EMBEDDINGS_IDX: &str =
    "CREATE INDEX IF NOT EXISTS idx_fe_file ON face_embeddings(file_id, mtime);";

pub const CREATE_FACE_CENTROIDS: &str = "
CREATE TABLE IF NOT EXISTS face_centroids (
    cluster_id  TEXT PRIMARY KEY,
    vec         BLOB NOT NULL
);
";

pub const CREATE_LIBRARY_ROOTS: &str = "
CREATE TABLE IF NOT EXISTS library_roots (
    path        TEXT PRIMARY KEY,
    recursive   INTEGER NOT NULL DEFAULT 1,
    added_at    INTEGER NOT NULL
);
";

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
    // Migrate tags.origin (TEXT) → tags.sources (INTEGER bitmask: ai=1, xmp=2, apple=4)
    "ALTER TABLE tags ADD COLUMN sources INTEGER NOT NULL DEFAULT 0",
    "UPDATE tags SET sources = CASE origin WHEN 'ai' THEN 1 WHEN 'xmp' THEN 2 WHEN 'apple' THEN 4 ELSE 0 END",
    "ALTER TABLE tags DROP COLUMN origin",
    // Drop sources — provenance tracking moved out of the tags table
    "ALTER TABLE tags DROP COLUMN sources",
    // AI auto-tagging removed — drop its suggestion-staging table.
    "DROP TABLE IF EXISTS pending_tags",
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
    CREATE_FACE_CLUSTERS,
    CREATE_FACE_CLUSTER_NAMES,
    CREATE_FACE_CLUSTER_IDX,
    CREATE_FACE_EMBEDDINGS,
    CREATE_FACE_EMBEDDINGS_IDX,
    CREATE_FACE_CENTROIDS,
    CREATE_LIBRARY_ROOTS,
];
