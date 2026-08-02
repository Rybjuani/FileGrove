-- Scheduled organization was removed from the Linux-first interface. Move
-- legacy folders to the safest inactive mode without deleting their saved
-- schedule, so a future compatibility tool can still inspect it.
UPDATE folders
SET mode = 'single', watcher_status = 'stopped'
WHERE mode = 'scheduled';
