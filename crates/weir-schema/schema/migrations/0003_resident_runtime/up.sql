-- Logical migration (diesel-dualdb): resident source runtime ([[WEIR-I-0035]] F1 / WEIR-T-0137).
-- Adds the per-connection execution mode: `run_once` (default; today's batch/micro-batch/CDC path)
-- vs `resident` (a long-lived source that stays up and emits under supervision). Backward-compatible:
-- existing rows default to `run_once`. Regenerate per-backend SQL + schema.rs with `angreal schema gen`.
ALTER TABLE connections ADD COLUMN execution_mode TEXT NOT NULL DEFAULT 'run_once';
-- work_units carries the resolved `ExecutionMode` (enum-as-JSON, like `partition`) so the executor
-- sees the mode the connection was planned with. Existing rows default to run-once.
ALTER TABLE work_units ADD COLUMN execution_mode TEXT NOT NULL DEFAULT '{"mode":"run_once"}';
