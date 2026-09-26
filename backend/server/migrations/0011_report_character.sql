-- The character the player joined with, a copy from `report` for the list.
ALTER TABLE connection_reports ADD COLUMN character TEXT NOT NULL DEFAULT '';
