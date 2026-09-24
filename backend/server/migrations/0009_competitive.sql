-- A competitive server lets in nobody who carries items from a tier its world
-- has not reached, see docs/competitive.md. The game server reports its world
-- and its dead bosses, the join codes carry them to the players.
ALTER TABLE servers
    ADD COLUMN competitive BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN world TEXT,
    ADD COLUMN boss_keys TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN progress_at TIMESTAMPTZ;
