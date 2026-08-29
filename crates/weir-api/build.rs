//! Embed the prebuilt Leptos UI (`weir-ui/dist`) into the binary *if present*.
//! Generates `UI_FILES: &[(&str, &[u8])]` — empty when the UI hasn't been built,
//! so `cargo build` never requires trunk. Run `trunk build` in `weir-ui/` to
//! produce the real embedded UI.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weir-ui/dist");
    println!("cargo:rerun-if-changed={}", dist.display());
    println!("cargo:rerun-if-env-changed=WEIR_REQUIRE_UI");

    let mut code = String::from(
        "/// (path, bytes) for each embedded UI asset.\npub static UI_FILES: &[(&str, &[u8])] = &[\n",
    );
    if dist.is_dir() {
        collect(&dist, &dist, &mut code);
    } else {
        // Dev builds may skip the UI, but never silently in a distribution: a headless
        // release artifact is exactly the failure [[WEIR-T-0165]] exists to kill.
        // Release/image builds set WEIR_REQUIRE_UI=1 to turn this warning into an error.
        println!(
            "cargo:warning=weir-ui/dist not found — the web UI will NOT be embedded \
             (run `trunk build --release` in weir-ui/)"
        );
        if std::env::var_os("WEIR_REQUIRE_UI").is_some() {
            panic!(
                "WEIR_REQUIRE_UI is set but weir-ui/dist is missing — \
                 build the UI first: (cd weir-ui && trunk build --release)"
            );
        }
    }
    code.push_str("];\n");

    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out).join("ui_assets.rs"), code).expect("write ui_assets.rs");
}

fn collect(root: &Path, dir: &Path, code: &mut String) {
    let entries = std::fs::read_dir(dir).expect("read dist dir");
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect(root, &path, code);
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            writeln!(
                code,
                "    ({:?}, include_bytes!({:?})),",
                rel,
                path.to_string_lossy()
            )
            .unwrap();
        }
    }
}
