-- Reverse of 0002_auth ([[WEIR-I-0017]]).
DROP INDEX idx_audit_events_ts;
DROP TABLE audit_events;
DROP TABLE tenants;
DROP INDEX idx_api_keys_hash;
DROP TABLE api_keys;
