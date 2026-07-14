//! `weir-codegen <manifest.yaml> wasm <outdir>` — lower a declarative manifest
//! into a `fidius` WASM-guest connector crate ([[WEIR-A-0030]] WASM-always).

use std::path::Path;
use std::process::ExitCode;

use weir_manifest::Manifest;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: weir-codegen <manifest.yaml> wasm <outdir>");
        return ExitCode::from(2);
    }
    let yaml = match std::fs::read_to_string(&args[1]) {
        Ok(y) => y,
        Err(e) => {
            eprintln!("read {}: {e}", args[1]);
            return ExitCode::FAILURE;
        }
    };
    let manifest = match Manifest::from_yaml(&yaml) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("manifest error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let generated = match args[2].as_str() {
        "wasm" => weir_codegen::generate_wasm_guest_crate(&manifest),
        other => {
            eprintln!("unknown mode `{other}` (expected `wasm`)");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = generated.write_to(Path::new(&args[3])) {
        eprintln!("write crate: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "generated {} file(s) into {}",
        generated.files.len(),
        args[3]
    );
    ExitCode::SUCCESS
}
