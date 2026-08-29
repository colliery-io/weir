#!/usr/bin/env bash
# Build the wasm connectors and **pre-stage** them as fidius packages into an
# output dir (the distribution image's WEIR_CONNECTORS_DIR). WASM-always
# ([[WEIR-A-0030]]); built where they run ([[WEIR-A-0032]] — the image build *is*
# the execution environment). Usage: stage-connectors.sh <out-dir>
set -euo pipefail
OUT="${1:?usage: stage-connectors.sh <out-dir>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$OUT"

# crate_dir  wasm_file  package  capability(single, or empty)
stage() {
  local crate_dir="$1" wasm="$2" pkg="$3" cap="${4:-}"
  ( cd "$crate_dir" && cargo build --target wasm32-wasip2 --release )
  local dir="$OUT/$pkg"
  mkdir -p "$dir"
  cp "$crate_dir/target/wasm32-wasip2/release/$wasm" "$dir/$wasm"
  local caps="" ver
  [ -n "$cap" ] && caps="\"$cap\""
  ver="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$crate_dir/Cargo.toml" | head -1)"
  cat > "$dir/package.toml" <<EOF
[package]
name = "$pkg"
version = "$ver"
interface = "connector"
interface_version = 1
runtime = "wasm"

[metadata]
category = "connector"

[wasm]
component = "$wasm"
capabilities = [$caps]
EOF
  echo "  staged $pkg"
}

# The shared declarative runtime (rest, needs http) + destinations (arrow-sink + the
# real postgres sink, needs tcp) + the self-contained demo sources.
stage "$ROOT/crates/connectors/rest" weir_rest_wasm.wasm   weir-rest-pkg   http
stage "$ROOT/crates/connectors/s3" weir_s3_wasm.wasm       weir-s3-pkg     http
stage "$ROOT/crates/connectors/rest-dest" weir_rest_dest_wasm.wasm weir-rest-dest-pkg http
stage "$ROOT/crates/connectors/snowflake" weir_snowflake_wasm.wasm weir-snowflake-pkg http
stage "$ROOT/crates/connectors/postgres" weir_postgres_wasm.wasm weir-postgres-pkg tcp
stage "$ROOT/crates/connectors/mssql" weir_mssql_wasm.wasm weir-mssql-pkg tcp
stage "$ROOT/wasm-fixtures/arrow-sink"   weir_arrow_sink_wasm.wasm weir-arrow-sink-pkg
stage "$ROOT/wasm-fixtures/echo"         weir_echo_wasm.wasm       weir-echo-pkg
stage "$ROOT/wasm-fixtures/slow"         weir_slow_wasm.wasm       weir-slow-pkg
stage "$ROOT/wasm-fixtures/faulty"       weir_faulty_wasm.wasm     weir-faulty-pkg
echo "staged $(ls -1 "$OUT" | wc -l | tr -d ' ') connectors → $OUT"
