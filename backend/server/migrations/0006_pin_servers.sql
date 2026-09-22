-- Every pin names the server it is for. Pins saved before that, a mod whose
-- `requested` is a version and that has no `server`, were all made for Durka.
UPDATE cloud_revisions
SET setup = jsonb_set(
    setup::jsonb,
    '{mods}',
    (
        SELECT COALESCE(
            jsonb_object_agg(
                entry.key,
                CASE
                    WHEN jsonb_typeof(entry.value -> 'requested') = 'string'
                        AND entry.value ->> 'requested' <> '*'
                        AND jsonb_typeof(entry.value -> 'server') IS DISTINCT FROM 'string'
                    THEN entry.value || '{"server": "Durka"}'::jsonb
                    ELSE entry.value
                END
            ),
            '{}'::jsonb
        )
        FROM jsonb_each(setup::jsonb -> 'mods') AS entry
    )
)::text
WHERE jsonb_typeof(setup::jsonb -> 'mods') = 'object';

-- A pin finds its server by name, so a name belongs to one server of a game.
ALTER TABLE servers DROP CONSTRAINT servers_owner_id_game_name_key;
ALTER TABLE servers ADD CONSTRAINT servers_game_name_key UNIQUE (game, name);
