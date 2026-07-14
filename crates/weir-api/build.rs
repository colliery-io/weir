//! Embed the prebuilt Leptos UI (`weir-ui/dist`) into the binary *if present*.
//! Generates `UI_FILES: &[(&str, &[u8])]` — empty when the UI hasn't been built,
//! so `cargo build` never requires trunk. Run `trunk build` in `weir-ui/` to
//! produce the real embedded UI.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weir-ui/dist");
    println!("cargo:rerun-if-changed={}", dist.display());

    let mut code = String::from(
        "/// (path, bytes) for each embedded UI asset.\npub static UI_FILES: &[(&str, &[u8])] = &[\n",
    );
    if dist.is_dir() {
        collect(&dist, &dist, &mut code);
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
