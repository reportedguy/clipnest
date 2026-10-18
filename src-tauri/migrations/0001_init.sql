-- ClipNest schema v1.
-- Timestamps are Unix epoch milliseconds (UTC). Image paths are relative to the app data dir.

CREATE TABLE categories (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    hint        TEXT,
    color       TEXT    NOT NULL,
    icon        TEXT,
    sort_order  INTEGER NOT NULL,
    is_system   INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    created_at  INTEGER NOT NULL
);

-- System categories (Miscellaneous) can be recolored/reordered but never renamed or deleted.
CREATE TRIGGER categories_protect_system_delete
BEFORE DELETE ON categories WHEN OLD.is_system = 1
BEGIN
    SELECT RAISE(ABORT, 'system category cannot be deleted');
END;

CREATE TRIGGER categories_protect_system_rename
BEFORE UPDATE OF name, is_system ON categories
WHEN OLD.is_system = 1 AND (NEW.name IS NOT OLD.name OR NEW.is_system IS NOT OLD.is_system)
BEGIN
    SELECT RAISE(ABORT, 'system category cannot be renamed');
END;

CREATE TABLE screenshots (
    id              INTEGER PRIMARY KEY,
    category_id     INTEGER NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
    image_path      TEXT    NOT NULL,
    thumb_path      TEXT    NOT NULL,
    width           INTEGER NOT NULL,
    height          INTEGER NOT NULL,
    user_note       TEXT    NOT NULL DEFAULT '',
    ocr_text        TEXT,
    title           TEXT,
    summary         TEXT,
    tags_json       TEXT    NOT NULL DEFAULT '[]',
    urls_json       TEXT    NOT NULL DEFAULT '[]',
    mode            TEXT    NOT NULL DEFAULT 'text' CHECK (mode IN ('text', 'image')),
    auto_upgraded   INTEGER NOT NULL DEFAULT 0 CHECK (auto_upgraded IN (0, 1)),
    -- True when the user picked the category in the popup; the AI must not override it.
    category_locked INTEGER NOT NULL DEFAULT 0 CHECK (category_locked IN (0, 1)),
    status          TEXT    NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued', 'processing', 'done', 'failed')),
    error           TEXT,
    attempts        INTEGER NOT NULL DEFAULT 0,
    deadline_at     INTEGER,
    deadline_label  TEXT,
    deadline_done   INTEGER NOT NULL DEFAULT 0 CHECK (deadline_done IN (0, 1)),
    notified_at     INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX screenshots_by_category ON screenshots (category_id, created_at DESC);
CREATE INDEX screenshots_by_status   ON screenshots (status, created_at) WHERE status IN ('queued', 'processing');
CREATE INDEX screenshots_by_deadline ON screenshots (deadline_at) WHERE deadline_at IS NOT NULL AND deadline_done = 0;

-- External-content FTS index; triggers below keep it in sync with `screenshots`.
CREATE VIRTUAL TABLE screenshots_fts USING fts5 (
    title, summary, user_note, ocr_text, tags_json,
    content = 'screenshots',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);

CREATE TRIGGER screenshots_fts_ai AFTER INSERT ON screenshots BEGIN
    INSERT INTO screenshots_fts (rowid, title, summary, user_note, ocr_text, tags_json)
    VALUES (NEW.id, NEW.title, NEW.summary, NEW.user_note, NEW.ocr_text, NEW.tags_json);
END;

CREATE TRIGGER screenshots_fts_ad AFTER DELETE ON screenshots BEGIN
    INSERT INTO screenshots_fts (screenshots_fts, rowid, title, summary, user_note, ocr_text, tags_json)
    VALUES ('delete', OLD.id, OLD.title, OLD.summary, OLD.user_note, OLD.ocr_text, OLD.tags_json);
END;

CREATE TRIGGER screenshots_fts_au
AFTER UPDATE OF title, summary, user_note, ocr_text, tags_json ON screenshots BEGIN
    INSERT INTO screenshots_fts (screenshots_fts, rowid, title, summary, user_note, ocr_text, tags_json)
    VALUES ('delete', OLD.id, OLD.title, OLD.summary, OLD.user_note, OLD.ocr_text, OLD.tags_json);
    INSERT INTO screenshots_fts (rowid, title, summary, user_note, ocr_text, tags_json)
    VALUES (NEW.id, NEW.title, NEW.summary, NEW.user_note, NEW.ocr_text, NEW.tags_json);
END;

-- Values are JSON-encoded so each setting keeps its type.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Default categories. Hints are passed to the AI to improve filing accuracy.
INSERT INTO categories (name, hint, color, sort_order, is_system, created_at) VALUES
    ('Ideas',                     'content ideas, things I might make or write', '#f59e0b', 0, 0, CAST(strftime('%s','now') AS INTEGER) * 1000),
    ('Learning',                  'knowledge, explanations, tips, how-tos',      '#10b981', 1, 0, CAST(strftime('%s','now') AS INTEGER) * 1000),
    ('Resources',                 'tools, websites, libraries, references',      '#3b82f6', 2, 0, CAST(strftime('%s','now') AS INTEGER) * 1000),
    ('Opportunities & Deadlines', 'competitions, applications, events, jobs, anything with a date to act on', '#ef4444', 3, 0, CAST(strftime('%s','now') AS INTEGER) * 1000),
    ('Watch Later',               'videos, talks, courses, films to watch',      '#8b5cf6', 4, 0, CAST(strftime('%s','now') AS INTEGER) * 1000),
    ('Miscellaneous',             NULL,                                          '#78716c', 5, 1, CAST(strftime('%s','now') AS INTEGER) * 1000);
