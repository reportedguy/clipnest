-- Soft delete, so "Delete" can be undone. Rows are purged (with their files) a day later.
ALTER TABLE screenshots ADD COLUMN deleted_at INTEGER;

DROP INDEX screenshots_by_category;
CREATE INDEX screenshots_live_by_category ON screenshots (category_id, created_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX screenshots_deleted ON screenshots (deleted_at) WHERE deleted_at IS NOT NULL;
