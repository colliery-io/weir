ALTER TABLE connections ADD COLUMN execution_mode TEXT NOT NULL DEFAULT 'run_once';

ALTER TABLE work_units ADD COLUMN execution_mode TEXT NOT NULL DEFAULT '{"mode":"run_once"}';
