-- `users` comes from the auth migrations of hilen-server, which run first.

CREATE TABLE profiles (
    user_id    UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    -- Lower case, the rule is `blackforge_api::username`.
    username   TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE friend_requests (
    from_user  UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    to_user    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (from_user, to_user),
    CHECK (from_user <> to_user)
);

CREATE INDEX friend_requests_to_user ON friend_requests (to_user);

-- One row per pair, the smaller id first, so a friendship cannot exist twice.
CREATE TABLE friendships (
    user_a     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    user_b     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_a, user_b),
    CHECK (user_a < user_b)
);

CREATE INDEX friendships_user_b ON friendships (user_b);

-- `profile` is the JSON of `blackforge_api::SharedProfile`, stored as the app
-- sent it and handed to friends as it is.
CREATE TABLE shared_profiles (
    user_id    UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    profile    TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- In game means `in_game` and a `last_seen` younger than two minutes, the app
-- reports once a minute while the game runs.
CREATE TABLE game_status (
    user_id   UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    in_game   BOOLEAN NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL DEFAULT now()
);
