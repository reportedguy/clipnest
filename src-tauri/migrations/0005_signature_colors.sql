-- Move the default categories to the design system's signature palette. Only colors the
-- user never changed (still the original defaults) are updated.
UPDATE categories SET color = '#aa2d00' WHERE name = 'Ideas' AND color = '#f59e0b';
UPDATE categories SET color = '#0a2e0e' WHERE name = 'Learning' AND color = '#10b981';
UPDATE categories SET color = '#a8d8c4' WHERE name = 'Resources' AND color = '#3b82f6';
UPDATE categories SET color = '#d9a441' WHERE name = 'Opportunities & Deadlines' AND color = '#ef4444';
UPDATE categories SET color = '#fcab79' WHERE name = 'Watch Later' AND color = '#8b5cf6';
UPDATE categories SET color = '#9297a0' WHERE is_system = 1 AND color = '#78716c';
