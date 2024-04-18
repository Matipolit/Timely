-- Add migration script here
ALTER TABLE todos
ALTER COLUMN parent_id
DROP NOT NULL
