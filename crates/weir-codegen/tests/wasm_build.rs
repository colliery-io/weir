//! WEIR-T-0003 criterion 4 (WASM half): the generated self-contained guest
//! (`crates/connectors/rest`, produced by `weir-codegen … wasm`) compiles to a
//! WASM component. The dylib half is proven by the workspace build of
//! `weir-connector-rest`.

use std::path::Path;
use std::process::Command;

#[test]
fn generated_wasm_guest_builds_to_component() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/rest");
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&dir)
        .status()
        .expect("run cargo build --target wasm32-wasip2");
    assert!(status.success(), "generated wasm guest failed to build");
    assert!(
        dir.join("target/wasm32-wasip2/release/weir_rest_wasm.wasm")
            .exists(),
        "expected the wasm component artifact"
    );
}
