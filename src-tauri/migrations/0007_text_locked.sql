-- Set when the user edits a card's title or summary by hand: later AI runs (retry,
-- re-file) keep that text instead of overwriting it.
ALTER TABLE screenshots ADD COLUMN text_locked INTEGER NOT NULL DEFAULT 0 CHECK (text_locked IN (0, 1));
