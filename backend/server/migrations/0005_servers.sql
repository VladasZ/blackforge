-- A game server somebody registered, with the mods a player needs to join
-- it. `mods` is the JSON of `blackforge_api::servers::ServerMod` rows, stored
-- as the app sent it. No address and no password, on purpose.
CREATE TABLE servers (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id   UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    game       TEXT NOT NULL,
    name       TEXT NOT NULL,
    mods       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (owner_id, game, name)
);

CREATE INDEX servers_owner ON servers (owner_id);
