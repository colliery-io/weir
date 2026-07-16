# weir MSSQL source connector ([[WEIR-T-0161]])

A compiled WASM guest that reads SQL Server tables over TDS (tiberius over `fidius_guest::sockets::tcp`, driven runtime-free via a poll-Ready socket adapter + `futures::block_on`). Reads use `FOR JSON PATH` so the server produces JSON. Source-only: full-refresh + cursor-incremental (batch parity; CDC is a follow-up).

Config: `host`, `port` (default 1433), `database`, `user`, `password`, optional `table` (blank = stream name). Integration test: `angreal integration up` then `cargo test -p weir-engine --test wasm_mssql_engine -- --ignored --test-threads=1`.
