-- Deadlines are stored whenever one is found, but only "important" ones (the note shows
-- intent to act, or the user set it by hand) appear in the strip and trigger reminders.
ALTER TABLE screenshots ADD COLUMN deadline_raw TEXT;
ALTER TABLE screenshots ADD COLUMN deadline_important INTEGER NOT NULL DEFAULT 0 CHECK (deadline_important IN (0, 1));

DROP INDEX screenshots_by_deadline;
CREATE INDEX screenshots_upcoming ON screenshots (deadline_at)
    WHERE deadline_at IS NOT NULL AND deadline_important = 1 AND deadline_done = 0 AND deleted_at IS NULL;
