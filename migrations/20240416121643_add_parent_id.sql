-- Add migration script here
ALTER TABLE todos
ADD parent_id BIGSERIAL,
ADD CONSTRAINT fk_todos FOREIGN KEY(parent_id) REFERENCES todos(id)
