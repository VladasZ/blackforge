-- The search asks `username LIKE 'vla%'`. The unique index of `profiles` sorts
-- by the collation of the database, which a LIKE cannot use. This one sorts
-- byte by byte.
CREATE INDEX profiles_username_prefix ON profiles (username text_pattern_ops);
