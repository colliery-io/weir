//! Shared test helper (WEIR-I-0011 S3): build a wasm `ConnectorRef` for a connector
//! name, staging the guest packages via the testkit (wasm-only; native retired).
use weir_orchestrator::{ConnectorRef, Origin};

pub fn wasm_ref(plugin: &str) -> ConnectorRef {
    let mut kebab = String::new();
    for (i, c) in plugin.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            kebab.push('-');
        }
        kebab.push(c.to_ascii_lowercase());
    }
    ConnectorRef::Wasm {
        search_path: weir_wasm_testkit::connectors_dir().display().to_string(),
        package: format!("weir-{kebab}-pkg"),
        version: "0.0.0".to_string(),
        origin: Origin::FirstParty,
    }
}
