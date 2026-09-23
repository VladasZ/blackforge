-- The public `ip:port` of a server's lobby, set only on servers of the join
-- admin. The app gives each such server a join button in the game menu.
ALTER TABLE servers ADD COLUMN address TEXT;
