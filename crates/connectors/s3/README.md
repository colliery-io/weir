# weir `s3` object-store source

Lists and reads **NDJSON** objects from an S3-compatible endpoint (AWS S3, MinIO, …) over HTTP.

- **Source** — `full_refresh` (all objects under the prefix) and `incremental` (object key as a
  lexicographic cursor via `start-after`, so date-partitioned / ULID key schemes resume cleanly).
- **Auth** — none in the guest. The host egress signs each request with **AWS SigV4**
  (`Credential::AwsSigV4`, [[WEIR-A-0033]]); the secret never enters the sandbox.

Config: `{ "endpoint": "...", "bucket": "...", "prefix": "..." }` plus the connection's
`auth_scheme: "aws_sigv4"` credentials (resolved host-side).
