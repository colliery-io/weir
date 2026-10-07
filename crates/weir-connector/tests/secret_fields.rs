//! Drift guard ([[WEIR-T-0222]]): every first-party compiled connector must mark its
//! credential-bearing `config_schema` properties as secret (`airbyte_secret: true` or
//! `format: "password"`). The connection API redacts a config value only when the schema
//! marks it ([[WEIR-T-0201]]), so an unmarked `password` leaks through the API.
//!
//! The test reads the `config_schema` literal straight from each guest's `src/lib.rs`
//! (the guests are wasm32 crates outside this workspace, so they cannot be called here).
//!
//! Declarative manifests (`manifests/`, `dest-manifests/`) register `config_schema: "{}"`;
//! their credential keys come from the baked `auth_*` metadata, which the redaction keys on
//! separately. They have no schema properties to mark.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Name fragments (lowercase) that make a property a credential.
const CREDENTIAL_FRAGMENTS: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "token",
    "credential",
    "api_key",
    "apikey",
    "access_key",
    "private_key",
    "privatekey",
];

/// Properties that hold a credential by content, not by name.
const CREDENTIAL_BY_CONTENT: &[(&str, &str)] = &[
    // A postgres URL can embed the password (`postgres://user:pass@host/db`).
    ("postgres", "url"),
];

fn is_credential_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "key" || n.ends_with("_key") || CREDENTIAL_FRAGMENTS.iter().any(|f| n.contains(f))
}

fn is_marked_secret(prop: &Value) -> bool {
    prop.get("airbyte_secret").and_then(Value::as_bool) == Some(true)
        || prop.get("format").and_then(Value::as_str) == Some("password")
}

/// The credential properties of `schema` that are not marked secret.
fn unmarked_credentials(connector: &str, schema: &Value) -> Vec<String> {
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    props
        .iter()
        .filter(|(name, prop)| {
            let credential = is_credential_name(name)
                || CREDENTIAL_BY_CONTENT.contains(&(connector, name.as_str()));
            credential && !is_marked_secret(prop)
        })
        .map(|(name, _)| name.clone())
        .collect()
}

/// Decode the Rust string literal that starts right after the opening `"` in `src`.
/// Handles `\"`, `\\`, `\n`, `\t` and the `\<newline><indent>` line continuation.
fn decode_rust_str(src: &str) -> String {
    let mut out = String::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => return out,
            '\\' => match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\n') => {
                    while chars.peek().is_some_and(|c| c.is_whitespace()) {
                        chars.next();
                    }
                }
                Some(other) => panic!("unsupported escape `\\{other}` in config_schema literal"),
                None => break,
            },
            c => out.push(c),
        }
    }
    panic!("unterminated config_schema literal");
}

fn connectors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../connectors")
}

/// `(connector, parsed config_schema)` for each first-party compiled guest.
fn first_party_schemas() -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let mut dirs: Vec<_> = std::fs::read_dir(connectors_dir())
        .expect("read crates/connectors")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.join("src/lib.rs").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(dir.join("src/lib.rs")).expect("read lib.rs");
        let marker = "config_schema: \"";
        let at = src
            .find(marker)
            .unwrap_or_else(|| panic!("{name}: no `config_schema: \"…\"` literal in src/lib.rs"));
        let literal = decode_rust_str(&src[at + marker.len()..]);
        let schema: Value = serde_json::from_str(&literal)
            .unwrap_or_else(|e| panic!("{name}: config_schema is not JSON: {e}\n{literal}"));
        out.push((name, schema));
    }
    out
}

#[test]
fn every_first_party_credential_field_is_marked_secret() {
    let schemas = first_party_schemas();
    for expected in ["mssql", "postgres", "rest", "rest-dest", "s3", "snowflake"] {
        assert!(
            schemas.iter().any(|(n, _)| n == expected),
            "first-party connector `{expected}` not found under crates/connectors"
        );
    }
    let gaps: Vec<String> = schemas
        .iter()
        .flat_map(|(name, schema)| {
            unmarked_credentials(name, schema)
                .into_iter()
                .map(move |p| format!("{name}.{p}"))
        })
        .collect();
    assert!(
        gaps.is_empty(),
        "credential fields not marked secret (add `\"airbyte_secret\":true`): {gaps:?}"
    );
}

#[test]
fn mssql_and_postgres_passwords_are_marked() {
    let schemas = first_party_schemas();
    for (connector, prop) in [
        ("mssql", "password"),
        ("postgres", "password"),
        ("postgres", "url"),
    ] {
        let (_, schema) = schemas.iter().find(|(n, _)| n == connector).unwrap();
        let p = &schema["properties"][prop];
        assert!(
            is_marked_secret(p),
            "{connector}.{prop} must be marked secret: {p}"
        );
    }
}

#[test]
fn the_guard_catches_an_unmarked_credential() {
    let schema = serde_json::json!({"type": "object", "properties": {
        "host": {"type": "string"},
        "password": {"type": "string"},
        "client_secret": {"type": "string", "airbyte_secret": true},
        "refresh_token": {"type": "string", "format": "password"},
        "signing_key": {"type": "string"},
        "max_keys": {"type": "integer"}
    }});
    let mut gaps = unmarked_credentials("example", &schema);
    gaps.sort();
    assert_eq!(
        gaps,
        vec!["password".to_string(), "signing_key".to_string()]
    );
}

#[test]
fn the_decoder_reads_line_continuations() {
    let src = "{\\\"a\\\":1,\\\n                \\\"b\\\":\\\"x\\\"}\".to_string()";
    assert_eq!(decode_rust_str(src), r#"{"a":1,"b":"x"}"#);
}
