//! WEIR-T-0013: migration fidelity. Importing the Airbyte declarative manifest
//! and running it through codegen yields the **byte-identical** connector source
//! that `weir-engine/tests/rest.rs` already compiles and paginates against a
//! mock HTTP server — so "Airbyte YAML → running weir connector" is proven by
//! composition with that green end-to-end test, no hand-editing.

use weir_importer::import_yaml;
use weir_manifest::{ArrowType, Manifest, Pagination};

// Local test fixtures (relocated out of manifests/, which is the vendored picker
// corpus — these are fidelity inputs, not onboardable connectors).
const AIRBYTE: &str = include_str!("fixtures/airbyte-rest.yaml");
const REST_REF: &str = include_str!("fixtures/rest.yaml");

#[test]
fn airbyte_imports_to_the_runnable_rest_connector() {
    let imported = import_yaml("rest", AIRBYTE).expect("import airbyte manifest");

    // The mapping captured the meaningful fields.
    assert_eq!(imported.base_url, "https://example.invalid");
    let s = &imported.streams[0];
    assert_eq!(s.path, "/posts");
    assert_eq!(s.primary_key, vec!["id".to_string()]);
    assert_eq!(s.incremental.as_ref().unwrap().cursor_param, "since");
    match s.pagination.as_ref().unwrap() {
        Pagination::Page {
            page_param,
            size_param,
            size,
        } => {
            assert_eq!(page_param, "_page");
            assert_eq!(size_param, "_limit");
            assert_eq!(*size, 2);
        }
        other => panic!("expected page pagination, got {other:?}"),
    }
    let names: Vec<_> = s.schema.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["id", "title", "updated_at"]);
    assert_eq!(s.schema[2].ty, ArrowType::Timestamp);

    // Fidelity: codegen(import(airbyte)) == codegen(rest manifest), byte for
    // byte. The wasm guest is what `wasm_http_engine` runs end-to-end → the Airbyte
    // path produces a running connector with no manual edits (WASM-always, A-0030).
    let reference = Manifest::from_yaml(REST_REF).expect("rest manifest");
    let gen_imported = weir_codegen::generate_wasm_guest_crate(&imported);
    let gen_reference = weir_codegen::generate_wasm_guest_crate(&reference);
    assert_eq!(
        gen_imported.file("src/lib.rs"),
        gen_reference.file("src/lib.rs"),
        "Airbyte→import→codegen must yield the exact connector rest.rs runs"
    );
}
