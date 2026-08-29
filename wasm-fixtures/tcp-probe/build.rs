// Regenerate wit/ + author<->component conversions from src/lib.rs (the
// #[derive(WitType)] typed-method types). A proc-macro can't see type defs, so
// this build step produces the WIT the #[plugin_impl] adapter consumes.
fn main() {
    fidius_build::emit_wit();
}
