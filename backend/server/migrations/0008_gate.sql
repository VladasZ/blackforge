-- Who may join the game servers of an owner. One list covers every server of
-- the owner. The owner is let in without a row.
CREATE TABLE server_members (
    owner_id   UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    member_id  UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (owner_id, member_id)
);

-- One time codes a player trades for a join. Only the sha256 of a code is
-- kept, a leaked table lets nobody in. A use deletes the row.
CREATE TABLE join_codes (
    code_hash  BYTEA PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    server_id  UUID NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX join_codes_expires ON join_codes (expires_at);
