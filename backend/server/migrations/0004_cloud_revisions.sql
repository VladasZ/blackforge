CREATE TABLE cloud_revisions (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    revision bigint NOT NULL CHECK (revision > 0),
    setup text NOT NULL,
    machine text NOT NULL DEFAULT '',
    -- Only applied revisions can be the head that machines install.
    applied boolean NOT NULL DEFAULT true,
    restored_from bigint,
    summary text NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, revision)
);

INSERT INTO cloud_revisions (user_id, revision, setup, created_at)
SELECT user_id, revision, setup, updated_at FROM cloud_setups;

DROP TABLE cloud_setups;
