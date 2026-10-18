-- A screenshot can now belong to several categories (up to 3), plus free-form tags.
--
-- `screenshot_categories` holds every membership. `screenshots.category_id` stays as the
-- card's *primary* category (the best match, shown first); the app keeps it pointing at
-- one of the card's categories. Keeping it avoids rebuilding the table (and its FTS
-- triggers) and keeps the existing foreign key meaningful.
CREATE TABLE screenshot_categories (
    screenshot_id INTEGER NOT NULL REFERENCES screenshots(id) ON DELETE CASCADE,
    category_id   INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    PRIMARY KEY (screenshot_id, category_id)
) WITHOUT ROWID;
CREATE INDEX screenshot_categories_by_category ON screenshot_categories (category_id, screenshot_id);

INSERT INTO screenshot_categories (screenshot_id, category_id) SELECT id, category_id FROM screenshots;

-- Tags, normalized for fast filtering. `tags_json` stays the source of truth (it feeds
-- full-text search); these triggers mirror it.
CREATE TABLE screenshot_tags (
    screenshot_id INTEGER NOT NULL REFERENCES screenshots(id) ON DELETE CASCADE,
    tag           TEXT    NOT NULL COLLATE NOCASE,
    PRIMARY KEY (screenshot_id, tag)
) WITHOUT ROWID;
CREATE INDEX screenshot_tags_by_tag ON screenshot_tags (tag, screenshot_id);

INSERT OR IGNORE INTO screenshot_tags (screenshot_id, tag)
SELECT s.id, lower(trim(j.value)) FROM screenshots s, json_each(s.tags_json) j WHERE trim(j.value) != '';

CREATE TRIGGER screenshot_tags_ai AFTER INSERT ON screenshots BEGIN
    INSERT OR IGNORE INTO screenshot_tags (screenshot_id, tag)
    SELECT NEW.id, lower(trim(value)) FROM json_each(NEW.tags_json) WHERE trim(value) != '';
END;

CREATE TRIGGER screenshot_tags_au AFTER UPDATE OF tags_json ON screenshots BEGIN
    DELETE FROM screenshot_tags WHERE screenshot_id = NEW.id;
    INSERT OR IGNORE INTO screenshot_tags (screenshot_id, tag)
    SELECT NEW.id, lower(trim(value)) FROM json_each(NEW.tags_json) WHERE trim(value) != '';
END;

-- The home feed lists every screenshot newest first.
CREATE INDEX screenshots_feed ON screenshots (created_at DESC, id DESC) WHERE deleted_at IS NULL;
