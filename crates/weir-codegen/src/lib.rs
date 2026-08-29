//! # weir-codegen
//!
//! Lowers a [`weir_manifest::Manifest`] into a `fidius` `Connector` crate
//! ([[WEIR-A-0014]] §2 — codegen, not a runtime interpreter). WASM-always
//! ([[WEIR-A-0030]]): the only target is [`generate_wasm_guest_crate`] — a
//! **self-contained** WASM guest (its own interface + WitType types + `emit_wit`),
//! which is exactly why codegen is needed: `fidius_build::emit_wit` only reads the
//! guest's own source. (The native/dylib target was retired with [[WEIR-I-0011]].)

use std::path::PathBuf;

use weir_manifest::{Manifest, Pagination, Stream};

mod wasm;

/// A generated crate: a set of (relative path, file contents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCrate {
    pub files: Vec<(PathBuf, String)>,
}

impl GeneratedCrate {
    /// Write all files under `dir`, creating parent directories.
    pub fn write_to(&self, dir: &std::path::Path) -> std::io::Result<()> {
        for (rel, contents) in &self.files {
            let path = dir.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, contents)?;
        }
        Ok(())
    }

    /// Convenience: the contents of a generated file by relative path.
    pub fn file(&self, rel: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(p, _)| p.as_os_str() == rel)
            .map(|(_, c)| c.as_str())
    }
}

/// Generate the self-contained WASM-guest connector crate.
pub fn generate_wasm_guest_crate(m: &Manifest) -> GeneratedCrate {
    wasm::generate(m)
}

// ---- shared helpers ----

/// PascalCase identifier from a manifest name (`example-rest` → `ExampleRest`).
/// This is the connector struct name and thus its fidius plugin name.
pub(crate) fn pascal_case(name: &str) -> String {
    name.split(['-', '_', ' '])
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// snake_case identifier (`posts-v2` → `posts_v2`) for generated fn names.
pub(crate) fn snake_case(name: &str) -> String {
    name.chars()
        .map(|c| if c == '-' || c == ' ' { '_' } else { c })
        .collect::<String>()
        .to_lowercase()
}

/// Render a Rust string literal for an arbitrary value (quotes + escapes).
pub(crate) fn rust_str(s: &str) -> String {
    format!("{s:?}")
}

/// Rust expression selecting the record array from the parsed body `val`,
/// honoring an optional single-level `record_selector`.
pub(crate) fn selector_expr(stream: &Stream) -> String {
    match stream.record_selector.as_deref().filter(|s| !s.is_empty()) {
        None => "val.as_array().cloned()".to_string(),
        Some(sel) => format!(
            "val.get({}).and_then(|v| v.as_array().cloned())",
            rust_str(sel)
        ),
    }
}

/// Page-size for the stop condition, or `usize::MAX` (no pagination → one page).
pub(crate) fn page_size(p: &Option<Pagination>) -> u32 {
    match p {
        Some(Pagination::Page { size, .. }) | Some(Pagination::Offset { size, .. }) => *size,
        // Cursor pagination has no fixed page size; legacy codegen treats it as single-page.
        Some(Pagination::Cursor { .. }) | Some(Pagination::LinkHeader) | None => u32::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        Manifest::from_yaml(
            "spec: { name: rest }\nbase_url: https://api.example.com\nstreams:\n  \
             - name: posts\n    path: /posts\n    primary_key: [id]\n    \
             schema: [ { name: id, type: int64 }, { name: updated_at, type: timestamp } ]\n    \
             incremental: { cursor_field: updated_at, cursor_param: since }\n    \
             pagination: { kind: page, page_param: _page, size_param: _limit, size: 2 }\n",
        )
        .unwrap()
    }

    #[test]
    fn wasm_generation_shape() {
        let g = generate_wasm_guest_crate(&sample());
        let lib = g.file("src/lib.rs").expect("lib.rs");
        assert!(lib.contains(
            r#"#[plugin_impl(Connector, crate = "fidius_guest", config = weir_connector_types::Config)]"#
        ));
        assert!(lib.contains("-> fidius_guest::Stream<ReadMessage>")); // streaming read (v1)
        // The contract block is an external module now, sourced from the one canonical copy.
        assert!(lib.contains("mod weir_guest_types;"));
        assert!(
            !lib.contains("pub enum ReadMessage"),
            "contract block moved out of lib.rs"
        );
        assert!(lib.contains("DiscoverOutcome::Catalog"));
        assert!(lib.contains(r#"name: "posts".to_string()"#));
        assert!(g.file("build.rs").unwrap().contains("emit_wit"));
        assert!(g.file("Cargo.toml").unwrap().contains("[workspace]"));

        // Drift guard: the emitted contract file IS the canonical, and carries the current contract
        // (the CDC `Changes` variant the old inline template had silently dropped).
        let contract = g
            .file("src/weir_guest_types.rs")
            .expect("weir_guest_types.rs");
        assert_eq!(
            contract,
            crate::wasm::GUEST_CONTRACT,
            "guest contract must equal the canonical"
        );
        assert!(
            contract.contains("Changes(Vec<ChangeRecord>)"),
            "contract carries CDC Changes"
        );
        assert!(contract.contains("pub enum ReadMessage"));
    }

    /// Drift guard ([[WEIR-I-0031]] / [[WEIR-T-0136]] / [[WEIR-T-0172]]): every checked-in
    /// guest carries a `weir_guest_types.rs` byte-identical to the one canonical block.
    /// Guests are discovered by GLOB over the two guest roots — a new guest crate can never
    /// silently escape the guard the way mssql/snowflake/resident once did (they sat outside
    /// the old hand-list). If this fails, the fix is `angreal connectors sync-contract`
    /// (after intentionally editing `guest_contract.rs.in`).
    #[test]
    fn contract_drift() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let roots = [root.join("../connectors"), root.join("../../wasm-fixtures")];
        let mut checked = 0;
        for dir in roots {
            for entry in
                std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            {
                let path = entry.expect("dir entry").path();
                // A guest crate = a directory with Cargo.toml + src/ (skips the shared
                // workspace's Cargo.{toml,lock} files and target/).
                if !path.is_dir()
                    || !path.join("Cargo.toml").is_file()
                    || !path.join("src").is_dir()
                {
                    continue;
                }
                let f = path.join("src/weir_guest_types.rs");
                assert!(
                    f.is_file(),
                    "guest crate {} carries no src/weir_guest_types.rs — run `angreal connectors sync-contract`",
                    path.display()
                );
                let got = std::fs::read_to_string(&f)
                    .unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
                assert_eq!(
                    got,
                    crate::wasm::GUEST_CONTRACT,
                    "{} drifted from the canonical — run `angreal connectors sync-contract`",
                    path.display()
                );
                checked += 1;
            }
        }
        // 6 connectors (postgres, rest, rest-dest, s3, mssql, snowflake) + 6 fixtures
        // (echo, slow, faulty, arrow-sink, resident, tcp-probe) at the time of writing.
        assert!(
            checked >= 11,
            "glob found only {checked} guests — roots moved?"
        );
    }
}
