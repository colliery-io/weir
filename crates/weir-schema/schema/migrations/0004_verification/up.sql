-- Logical migration (diesel-dualdb): connector verification record ([[WEIR-T-0183]]).
-- `verified_at` = when this connector last passed the LIVE suite against its real
-- API (ISO 8601 date, from manifests/verified.json at registration). NULL =
-- unverified — the honest default. Regenerate with `angreal schema gen`.
ALTER TABLE connectors ADD COLUMN verified_at TEXT;
