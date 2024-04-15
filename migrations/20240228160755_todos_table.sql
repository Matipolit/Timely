-- Add migration script here
CREATE TABLE IF NOT EXISTS todos
(
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT    NOT NULL,
    done        BOOLEAN NOT NULL DEFAULT FALSE,
    description TEXT,
    parent_id      BIGSERIAL,
    CONSTRAINT fk_todos FOREIGN KEY(parent_id) REFERENCES todos(id)
);
