-- What the game of a player saw when a join failed or a connection dropped,
-- see docs/reports.md. The whole report with its logs is in `report`, the
-- other columns are copies for the list.
CREATE TABLE connection_reports (
    id         BIGSERIAL PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    server     TEXT NOT NULL,
    status     TEXT NOT NULL,
    message    TEXT NOT NULL,
    in_world   BOOLEAN NOT NULL,
    seconds    DOUBLE PRECISION NOT NULL,
    report     JSONB NOT NULL
);

CREATE INDEX connection_reports_created ON connection_reports (created_at DESC);
