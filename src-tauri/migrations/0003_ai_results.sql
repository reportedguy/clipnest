-- The model's raw (validated) answer, kept so deadlines and future features can be
-- re-derived without re-running the AI, plus which model/prompt produced it.
ALTER TABLE screenshots ADD COLUMN ai_json TEXT;
ALTER TABLE screenshots ADD COLUMN ai_model TEXT;

-- Set by "Re-check Miscellaneous": the item is re-filed but only moved if the AI now
-- picks a real category; its title and summary are left alone.
ALTER TABLE screenshots ADD COLUMN recheck INTEGER NOT NULL DEFAULT 0 CHECK (recheck IN (0, 1));
