-- Add migration script here
ALTER TABLE todos
DROP CONSTRAINT IF EXISTS fk_todos
