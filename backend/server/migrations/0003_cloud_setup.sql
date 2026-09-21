CREATE TABLE cloud_setups (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    revision bigint NOT NULL CHECK (revision > 0),
    setup text NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
