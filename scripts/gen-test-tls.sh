#!/usr/bin/env bash
# Generate the EPHEMERAL TLS material for the integration TLS harness
# ([[WEIR-T-0178]] / [[WEIR-A-0041]] wire-proof gates): a throwaway CA, a
# localhost-SAN server leaf, a hostssl-only pg_hba, and an mssql.conf with
# Force Encryption. Nothing here is ever committed (default out-dir lives
# under target/); certs are short-lived by design.
# Usage: gen-test-tls.sh [out-dir]   (default: target/weir-tls-certs)
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/target/weir-tls-certs}"
mkdir -p "$OUT"

# Idempotent: reuse material younger than a day (compose restarts shouldn't churn).
if [ -f "$OUT/ca.crt" ] && [ -n "$(find "$OUT/ca.crt" -mtime -1 2>/dev/null)" ]; then
  echo "tls material fresh in $OUT"
  exit 0
fi

openssl req -x509 -newkey rsa:2048 -keyout "$OUT/ca.key" -out "$OUT/ca.crt" \
  -days 2 -nodes -subj "/CN=weir-test-ca" 2>/dev/null
openssl req -newkey rsa:2048 -keyout "$OUT/server.key" -out "$OUT/server.csr" \
  -nodes -subj "/CN=localhost" 2>/dev/null
openssl x509 -req -in "$OUT/server.csr" -CA "$OUT/ca.crt" -CAkey "$OUT/ca.key" \
  -CAcreateserial -out "$OUT/server.crt" -days 2 \
  -extfile <(printf "subjectAltName=DNS:localhost") 2>/dev/null
chmod 644 "$OUT/server.crt" "$OUT/ca.crt"
chmod 600 "$OUT/server.key" "$OUT/ca.key"

# hostssl-ONLY pg_hba: a plaintext TCP connection is REJECTED, so any passing
# sync against this server proves TLS actually engaged. `local` stays open for
# initdb + the in-container healthcheck.
cat > "$OUT/pg_hba.conf" <<'EOF'
local   all all           trust
hostssl all all 0.0.0.0/0 scram-sha-256
hostssl all all ::0/0     scram-sha-256
EOF

# SQL Server Force Encryption ([[WEIR-T-0177]]'s gate).
cat > "$OUT/mssql.conf" <<'EOF'
[network]
tlscert = /certs/server.crt
tlskey = /certs/server.key
tlsprotocols = 1.2
forceencryption = 1
EOF

echo "tls material generated in $OUT"
