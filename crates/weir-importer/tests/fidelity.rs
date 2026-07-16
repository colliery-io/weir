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
            ..
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

/// Body-injection fidelity ([[WEIR-T-0154]]): an Airbyte manifest whose paginator token
/// and incremental lower bound inject into the POST body (`inject_into: body_json`,
/// nested `field_path`) imports to exactly the weir manifest a connector author would
/// hand-write with `inject_into: body`.
#[test]
fn airbyte_body_injection_imports_to_the_handwritten_manifest() {
    let airbyte = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: search
    primary_key: id
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.notion.example"
        path: "/v1/search"
        http_method: POST
        request_body_json:
          filter: { property: object, value: page }
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: ["results"]
      paginator:
        type: DefaultPaginator
        pagination_strategy:
          type: CursorPagination
          cursor_value: "{{ response['next_cursor'] }}"
        page_token_option:
          type: RequestOption
          inject_into: body_json
          field_name: start_cursor
    incremental_sync:
      type: DatetimeBasedCursor
      cursor_field: last_edited_time
      start_time_option:
        type: RequestOption
        inject_into: body_json
        field_path: ["filter", "timestamp_after"]
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: string }
          last_edited_time: { type: string, format: date-time }
"#;
    let handwritten = r#"
spec: { name: search-src }
base_url: https://api.notion.example
streams:
  - name: search
    path: /v1/search
    primary_key: [id]
    http_method: POST
    request_body: '{"filter":{"property":"object","value":"page"}}'
    record_selector: results
    schema:
      - { name: id, type: utf8, nullable: false }
      - { name: last_edited_time, type: timestamp, nullable: false }
    incremental:
      cursor_field: last_edited_time
      cursor_param: filter.timestamp_after
      inject_into: body
    pagination:
      kind: cursor
      cursor_path: next_cursor
      token_param: start_cursor
      inject_into: body
"#;
    let imported = import_yaml("search-src", airbyte).expect("import body-injected manifest");
    let reference = Manifest::from_yaml(handwritten).expect("handwritten manifest");
    assert_eq!(
        imported, reference,
        "Airbyte body_json injection must lower to the hand-written inject_into: body form"
    );
}
